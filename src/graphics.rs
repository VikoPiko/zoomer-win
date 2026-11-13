use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextApi, ContextAttributesBuilder, GlProfile, PossiblyCurrentContext, Version},
    display::{Display, DisplayApiPreference},
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use glutin_winit::GlWindow;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{cell::Cell, ffi::CString, num::NonZeroU32};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event_loop::ActiveEventLoop,
    platform::windows::WindowAttributesExtWindows,
    window::{Fullscreen, Window, WindowLevel},
};
use zoomer::{
    annotation::Drawing,
    camera::{Camera, Point},
    capture::Frame,
    config::{Config, Filter},
};

#[derive(Debug, thiserror::Error)]
#[error("OpenGL viewer: {0}. Update your graphics driver and try again; OpenGL 3.3 is required.")]
pub struct GraphicsError(String);
fn error(e: impl std::fmt::Display) -> GraphicsError {
    GraphicsError(e.to_string())
}

/// Field order is intentional: GL objects die while the context and native window still exist.
pub struct Viewer {
    renderer: Renderer,
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
    pub window: Window,
}

impl Viewer {
    pub fn new(
        event_loop: &ActiveEventLoop,
        frame: &Frame,
        config: &Config,
    ) -> Result<Self, GraphicsError> {
        frame.validate().map_err(error)?;
        let d = frame.display;
        let monitor = event_loop
            .available_monitors()
            .find(|m| {
                let p = m.position();
                let s = m.size();
                p.x == d.x && p.y == d.y && s.width == d.width && s.height == d.height
            })
            .ok_or_else(|| error("captured monitor is no longer available"))?;
        let attrs = Window::default_attributes()
            .with_title("Zoomer")
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_skip_taskbar(true)
            .with_drag_and_drop(false)
            .with_undecorated_shadow(false)
            .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
            .with_position(PhysicalPosition::new(d.x, d.y))
            .with_inner_size(PhysicalSize::new(d.width, d.height));
        let window = event_loop.create_window(attrs).map_err(error)?;
        let raw_window = window.window_handle().map_err(error)?.as_raw();
        let raw_display = window.display_handle().map_err(error)?.as_raw();
        // SAFETY: Both raw handles belong to `window`, retained until after all GL resources.
        let display =
            unsafe { Display::new(raw_display, DisplayApiPreference::Wgl(Some(raw_window))) }
                .map_err(error)?;
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(0)
            .with_depth_size(0)
            .with_stencil_size(0)
            .compatible_with_native_window(raw_window)
            .build();
        // SAFETY: the template's native window remains alive on this event-loop thread.
        let gl_config = unsafe { display.find_configs(template) }
            .map_err(error)?
            .next()
            .ok_or_else(|| error("no compatible pixel format"))?;
        let attrs = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(raw_window));
        // SAFETY: configuration, display and window are compatible and live on this thread.
        let context = unsafe { display.create_context(&gl_config, &attrs) }.map_err(error)?;
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Err(error("window has zero framebuffer size"));
        }
        if size.width != d.width || size.height != d.height {
            return Err(error(
                "framebuffer dimensions differ from captured physical monitor; retry after display settings settle",
            ));
        }
        let attrs = window
            .build_surface_attributes(SurfaceAttributesBuilder::<WindowSurface>::new())
            .map_err(error)?;
        // SAFETY: attributes contain the live native window and nonzero framebuffer size.
        let surface =
            unsafe { display.create_window_surface(&gl_config, &attrs) }.map_err(error)?;
        let context = context.make_current(&surface).map_err(error)?;
        let _ = surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN));
        // SAFETY: WGL context is current; glutin provides valid entry points for its lifetime.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                CString::new(name).map_or(std::ptr::null(), |name| display.get_proc_address(&name))
            })
        };
        let renderer = Renderer::new(gl, frame, config.appearance.filter)?;
        Ok(Self {
            renderer,
            context,
            surface,
            window,
        })
    }

    pub fn draw(
        &self,
        camera: &Camera,
        cursor: Point,
        flashlight: bool,
        radius: f64,
        config: &Config,
        ink: &Drawing,
    ) -> Result<(), GraphicsError> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        self.window.resize_surface(&self.surface, &self.context);
        self.renderer
            .draw(size, camera, cursor, flashlight, radius, config)?;
        self.renderer.draw_ink(
            size,
            camera,
            ink,
            self.window.scale_factor().clamp(0.5, 4.0),
        )?;
        self.window.pre_present_notify();
        self.surface.swap_buffers(&self.context).map_err(error)
    }
    pub fn set_filter(&self, filter: Filter) {
        self.renderer.set_filter(filter);
    }
}

struct Renderer {
    gl: glow::Context,
    program: Option<glow::NativeProgram>,
    vao: Option<glow::NativeVertexArray>,
    texture: Option<glow::NativeTexture>,
    ink_program: Option<glow::NativeProgram>,
    ink_vao: Option<glow::NativeVertexArray>,
    ink_buffer: Option<glow::NativeBuffer>,
    ink_revision: Cell<u64>,
    ink_ui_scale: Cell<f64>,
    ink_count: Cell<i32>,
    image_size: PhysicalSize<u32>,
}

impl Renderer {
    fn new(gl: glow::Context, frame: &Frame, filter: Filter) -> Result<Self, GraphicsError> {
        let mut renderer = Self {
            gl,
            program: None,
            vao: None,
            texture: None,
            ink_program: None,
            ink_vao: None,
            ink_buffer: None,
            ink_revision: Cell::new(u64::MAX),
            ink_ui_scale: Cell::new(0.0),
            ink_count: Cell::new(0),
            image_size: PhysicalSize::new(frame.display.width, frame.display.height),
        };
        let gl = &renderer.gl;
        // SAFETY: the owning Viewer's GL context is current on the calling event-loop thread.
        // Every created object is immediately owned, and cleaned up even on partial failure.
        unsafe {
            let max = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE);
            if frame.display.width > max as u32 || frame.display.height > max as u32 || max <= 0 {
                return Err(error(format!(
                    "screen exceeds GPU maximum texture dimension {max}"
                )));
            }
            for (slot, vertex, fragment) in [
                (&mut renderer.program, VERTEX, FRAGMENT),
                (&mut renderer.ink_program, INK_VERTEX, INK_FRAGMENT),
            ] {
                let program = gl.create_program().map_err(error)?;
                *slot = Some(program);
                for (kind, source) in [
                    (glow::VERTEX_SHADER, vertex),
                    (glow::FRAGMENT_SHADER, fragment),
                ] {
                    let shader = gl.create_shader(kind).map_err(error)?;
                    gl.shader_source(shader, source);
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        let message = gl.get_shader_info_log(shader);
                        gl.delete_shader(shader);
                        return Err(error(message));
                    }
                    gl.attach_shader(program, shader);
                    // Attached shaders are deleted automatically when the program is disposed.
                    gl.delete_shader(shader);
                }
                gl.link_program(program);
                if !gl.get_program_link_status(program) {
                    return Err(error(gl.get_program_info_log(program)));
                }
            }
            renderer.vao = Some(gl.create_vertex_array().map_err(error)?);
            renderer.ink_vao = Some(gl.create_vertex_array().map_err(error)?);
            renderer.ink_buffer = Some(gl.create_buffer().map_err(error)?);
            gl.bind_vertex_array(renderer.ink_vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, renderer.ink_buffer);
            for (location, count, offset) in [(0, 2, 0), (1, 2, 8), (2, 2, 16), (3, 3, 24)] {
                gl.enable_vertex_attrib_array(location);
                gl.vertex_attrib_pointer_f32(location, count, glow::FLOAT, false, 36, offset);
                gl.vertex_attrib_divisor(location, 1);
            }
            renderer.texture = Some(gl.create_texture().map_err(error)?);
            gl.bind_texture(glow::TEXTURE_2D, renderer.texture);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                frame.display.width as i32,
                frame.display.height as i32,
                0,
                glow::BGRA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&frame.pixels)),
            );
            let code = gl.get_error();
            if code != glow::NO_ERROR {
                return Err(error(format!("texture upload failed (GL {code:#x})")));
            }
        }
        renderer.set_filter(filter);
        Ok(renderer)
    }
    fn set_filter(&self, filter: Filter) {
        let filter = match filter {
            Filter::Linear => glow::LINEAR,
            Filter::Nearest => glow::NEAREST,
        };
        // SAFETY: called only while the owning context is current; texture belongs to it.
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, self.texture);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
        }
    }
    fn draw(
        &self,
        size: PhysicalSize<u32>,
        camera: &Camera,
        cursor: Point,
        flashlight: bool,
        radius: f64,
        config: &Config,
    ) -> Result<(), GraphicsError> {
        let gl = &self.gl;
        // SAFETY: all objects belong to this current context, viewport is a nonzero window size.
        unsafe {
            gl.viewport(0, 0, size.width as i32, size.height as i32);
            gl.use_program(self.program);
            gl.bind_vertex_array(self.vao);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.texture);
            let uniform = |name| self.program.and_then(|p| gl.get_uniform_location(p, name));
            gl.uniform_1_i32(uniform("snapshot").as_ref(), 0);
            gl.uniform_2_f32(
                uniform("viewport").as_ref(),
                size.width as f32,
                size.height as f32,
            );
            gl.uniform_2_f32(
                uniform("image_size").as_ref(),
                self.image_size.width as f32,
                self.image_size.height as f32,
            );
            let p = camera.offset();
            gl.uniform_2_f32(uniform("translation").as_ref(), p.x as f32, p.y as f32);
            gl.uniform_1_f32(uniform("scale").as_ref(), camera.scale() as f32);
            gl.uniform_1_i32(uniform("mirror").as_ref(), i32::from(camera.mirror));
            gl.uniform_2_f32(uniform("cursor").as_ref(), cursor.x as f32, cursor.y as f32);
            gl.uniform_1_f32(uniform("radius").as_ref(), radius as f32);
            gl.uniform_1_f32(
                uniform("outside_brightness").as_ref(),
                config.flashlight.outside_brightness as f32,
            );
            gl.uniform_1_i32(uniform("flashlight").as_ref(), i32::from(flashlight));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            let code = gl.get_error();
            if code != glow::NO_ERROR {
                return Err(error(format!("drawing failed (GL {code:#x})")));
            }
        }
        Ok(())
    }
    fn draw_ink(
        &self,
        size: PhysicalSize<u32>,
        camera: &Camera,
        ink: &Drawing,
        ui_scale: f64,
    ) -> Result<(), GraphicsError> {
        let gl = &self.gl;
        // SAFETY: this renderer's context is current. The owned VBO/VAO define the
        // nine-f32 instance layout; bounded ink/UI data fits in i32 instance counts.
        unsafe {
            if self.ink_revision.get() != ink.revision() || self.ink_ui_scale.get() != ui_scale {
                let data = zoomer::pen_ui::instances(ink, ui_scale);
                let count = i32::try_from(data.len() / 9).map_err(error)?;
                // SAFETY: f32 is plain data, and the byte slice covers this live Vec only.
                let bytes = std::slice::from_raw_parts(
                    data.as_ptr().cast::<u8>(),
                    data.len() * size_of::<f32>(),
                );
                gl.bind_buffer(glow::ARRAY_BUFFER, self.ink_buffer);
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
                // Commit the cache only after a successful upload, allowing error recovery.
                let code = gl.get_error();
                if code != glow::NO_ERROR {
                    return Err(error(format!("pen upload failed (GL {code:#x})")));
                }
                self.ink_count.set(count);
                self.ink_revision.set(ink.revision());
                self.ink_ui_scale.set(ui_scale);
            }
            gl.use_program(self.ink_program);
            gl.bind_vertex_array(self.ink_vao);
            let uniform = |name| {
                self.ink_program
                    .and_then(|p| gl.get_uniform_location(p, name))
            };
            gl.uniform_2_f32(
                uniform("viewport").as_ref(),
                size.width as f32,
                size.height as f32,
            );
            gl.uniform_1_f32(
                uniform("image_width").as_ref(),
                self.image_size.width as f32,
            );
            let p = camera.offset();
            gl.uniform_2_f32(uniform("translation").as_ref(), p.x as f32, p.y as f32);
            gl.uniform_1_f32(uniform("scale").as_ref(), camera.scale() as f32);
            gl.uniform_1_i32(uniform("mirror").as_ref(), i32::from(camera.mirror));
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.ink_count.get());
            gl.disable(glow::BLEND);
            let code = gl.get_error();
            if code != glow::NO_ERROR {
                return Err(error(format!("pen drawing failed (GL {code:#x})")));
            }
        }
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: Viewer's field order retains the current context until after this drop.
        unsafe {
            self.gl.use_program(None);
            self.gl.bind_vertex_array(None);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            if let Some(buffer) = self.ink_buffer.take() {
                self.gl.delete_buffer(buffer);
            }
            if let Some(vao) = self.ink_vao.take() {
                self.gl.delete_vertex_array(vao);
            }
            if let Some(program) = self.ink_program.take() {
                self.gl.delete_program(program);
            }
            if let Some(texture) = self.texture.take() {
                self.gl.delete_texture(texture);
            }
            if let Some(vao) = self.vao.take() {
                self.gl.delete_vertex_array(vao);
            }
            if let Some(program) = self.program.take() {
                self.gl.delete_program(program);
            }
        }
    }
}

// Fullscreen triangle needs only an empty core-profile VAO, no vertex buffers.
const VERTEX: &str = r#"#version 330 core
void main() {
    vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

const FRAGMENT: &str = r#"#version 330 core
uniform sampler2D snapshot;
uniform vec2 viewport, image_size, translation, cursor;
uniform float scale, radius, outside_brightness;
uniform bool mirror, flashlight;
out vec4 color;
void main() {
    vec2 screen = vec2(gl_FragCoord.x, viewport.y - gl_FragCoord.y);
    vec2 image = (screen - translation) / scale;
    if (mirror) image.x = image_size.x - image.x;
    vec2 uv = image / image_size;
    vec3 rgb = vec3(0.055);
    if (all(greaterThanEqual(uv, vec2(0.0))) && all(lessThanEqual(uv, vec2(1.0))))
        rgb = texture(snapshot, uv).rgb;
    if (flashlight) {
        float feather = max(2.0, radius * 0.08);
        float edge = smoothstep(max(0.0, radius-feather), radius, distance(screen, cursor));
        rgb *= mix(1.0, outside_brightness, edge);
    }
    color = vec4(rgb, 1.0); // GDI's unused alpha byte is intentionally ignored.
}
"#;

const INK_VERTEX: &str = r#"#version 330 core
layout(location=0) in vec2 start;
layout(location=1) in vec2 end;
layout(location=2) in vec2 radius_space;
layout(location=3) in vec3 rgb;
uniform vec2 viewport,translation;
uniform float scale,image_width;
uniform bool mirror;
flat out vec2 a,b;
flat out float radius;
flat out vec3 ink_color;
void main(){
    a=start; b=end; radius=radius_space.x; ink_color=rgb;
    if(radius_space.y<0.5){
        if(mirror){a.x=image_width-a.x; b.x=image_width-b.x;}
        a=a*scale+translation; b=b*scale+translation; radius*=scale;
    }
    vec2 delta=b-a;
    float length_squared=dot(delta,delta);
    vec2 direction=length_squared>0.000001?delta*inversesqrt(length_squared):vec2(1.0,0.0);
    vec2 normal=vec2(-direction.y,direction.x);
    const vec2 corners[6]=vec2[6](vec2(0,-1),vec2(1,-1),vec2(1,1),vec2(0,-1),vec2(1,1),vec2(0,1));
    vec2 corner=corners[gl_VertexID];
    float extent=radius+1.0;
    vec2 p=mix(a-direction*extent,b+direction*extent,corner.x)+normal*corner.y*extent;
    gl_Position=vec4(p.x/viewport.x*2.0-1.0,1.0-p.y/viewport.y*2.0,0,1);
}
"#;
const INK_FRAGMENT: &str = r#"#version 330 core
uniform vec2 viewport;
flat in vec2 a,b;
flat in float radius;
flat in vec3 ink_color;
out vec4 color;
void main(){
    vec2 p=vec2(gl_FragCoord.x,viewport.y-gl_FragCoord.y);
    vec2 delta=b-a;
    float length_squared=dot(delta,delta);
    float t=length_squared>0.000001?clamp(dot(p-a,delta)/length_squared,0.0,1.0):0.0;
    float d=length(p-a-delta*t);
    float alpha=1.0-smoothstep(max(0.0,radius-0.75),radius+0.75,d);
    color=vec4(ink_color,alpha);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use winit::{
        application::ApplicationHandler, event::WindowEvent, event_loop::EventLoop,
        platform::windows::EventLoopBuilderExtWindows, window::WindowId,
    };
    use zoomer::capture::{self, PixelFormat};

    #[test]
    #[ignore = "requires a Windows desktop with OpenGL 3.3; uses generated pixels only"]
    fn hidden_renderer_color_orientation_and_mirror() {
        if crate::platform::gui_test_in_child(
            "graphics::tests::hidden_renderer_color_orientation_and_mirror",
        ) {
            return;
        }
        struct Smoke {
            result: Option<Result<(), GraphicsError>>,
        }
        impl ApplicationHandler for Smoke {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                self.result = Some((|| {
                    let (display, _) = capture::windows::target_under_cursor().map_err(error)?;
                    let (stride, size) =
                        capture::buffer_layout(display.width, display.height).map_err(error)?;
                    let mut pixels = vec![0; size];
                    for y in 0..display.height {
                        for x in 0..display.width {
                            let bgra = match (x < display.width / 2, y < display.height / 2) {
                                (true, true) => [0, 0, 255, 0],
                                (false, true) => [0, 255, 0, 0],
                                (true, false) => [255, 0, 0, 0],
                                (false, false) => [255, 255, 255, 0],
                            };
                            let offset = y as usize * stride + x as usize * 4;
                            pixels[offset..offset + 4].copy_from_slice(&bgra);
                        }
                    }
                    let frame = Frame {
                        display,
                        stride,
                        format: PixelFormat::Bgra8,
                        pixels,
                    };
                    let config = Config::default();
                    let viewer = Viewer::new(event_loop, &frame, &config)?;
                    assert_eq!(viewer.window.is_visible(), Some(false));
                    let size = viewer.window.inner_size();
                    let mut camera = Camera::new(&config.camera);
                    for (mirrored, flashlight) in [(false, false), (true, false), (true, true)] {
                        camera.mirror = mirrored;
                        viewer.renderer.draw(
                            size,
                            &camera,
                            Point::new(f64::from(size.width) / 2.0, f64::from(size.height) / 2.0),
                            flashlight,
                            240.0,
                            &config,
                        )?;
                        let mut pixel = [0; 4];
                        // SAFETY: this test owns the current context. Read only four bytes
                        // of the generated backbuffer, never the desktop or a captured image.
                        unsafe {
                            viewer.renderer.gl.read_pixels(
                                1,
                                size.height as i32 - 2,
                                1,
                                1,
                                glow::RGBA,
                                glow::UNSIGNED_BYTE,
                                glow::PixelPackData::Slice(Some(&mut pixel)),
                            );
                        }
                        if flashlight {
                            assert_eq!([pixel[0], pixel[2], pixel[3]], [0, 0, 255]);
                            assert!((70..=72).contains(&pixel[1]), "dimmed green: {pixel:?}");
                        } else {
                            assert_eq!(
                                pixel,
                                if mirrored {
                                    [0, 255, 0, 255]
                                } else {
                                    [255, 0, 0, 255]
                                }
                            );
                        }
                    }
                    let read_pixel = |screen: Point| {
                        let mut pixel = [0; 4];
                        // SAFETY: live context and a four-byte readback of generated pixels.
                        unsafe {
                            viewer.renderer.gl.read_pixels(
                                screen.x.floor() as i32,
                                size.height as i32 - 1 - screen.y.floor() as i32,
                                1,
                                1,
                                glow::RGBA,
                                glow::UNSIGNED_BYTE,
                                glow::PixelPackData::Slice(Some(&mut pixel)),
                            );
                        }
                        pixel
                    };
                    let mut ink = Drawing::default();
                    ink.toggle();
                    ink.set_color(2);
                    ink.change_width(4);
                    ink.begin(Point::new(300.5, 200.5), 1.0);
                    ink.append(Point::new(400.5, 200.5), 1.0);
                    ink.finish();
                    let center = Point::new(350.5, 200.5);
                    for mirrored in [false, true] {
                        camera.reset();
                        camera.mirror = mirrored;
                        viewer.renderer.draw(
                            size,
                            &camera,
                            Point::default(),
                            false,
                            240.0,
                            &config,
                        )?;
                        viewer.renderer.draw_ink(size, &camera, &ink, 1.0)?;
                        let pixel = read_pixel(camera.screen_at(center, f64::from(size.width)));
                        assert!(
                            (63..=65).contains(&pixel[0])
                                && pixel[1] == 255
                                && (101..=103).contains(&pixel[2]),
                            "ink color/mirror: {pixel:?}"
                        );
                    }
                    camera.drag(Point::new(80.0, 50.0), 0.016);
                    let anchor = camera.screen_at(center, f64::from(size.width));
                    camera.zoom(3.0, anchor);
                    for _ in 0..500 {
                        camera.advance(0.016, false);
                    }
                    viewer
                        .renderer
                        .draw(size, &camera, Point::default(), false, 240.0, &config)?;
                    viewer.renderer.draw_ink(size, &camera, &ink, 1.0)?;
                    assert_eq!(read_pixel(anchor)[1], 255, "ink must follow zoom/pan");
                    ink.undo();
                    viewer
                        .renderer
                        .draw(size, &camera, Point::default(), false, 240.0, &config)?;
                    viewer.renderer.draw_ink(size, &camera, &ink, 1.0)?;
                    assert_eq!(
                        read_pixel(anchor),
                        [255, 0, 0, 255],
                        "undo must remove uploaded ink"
                    );
                    Ok(())
                })());
                event_loop.exit();
            }
            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        }
        let event_loop = EventLoop::builder().with_any_thread(true).build().unwrap();
        let mut smoke = Smoke { result: None };
        event_loop.run_app(&mut smoke).unwrap();
        smoke.result.expect("renderer callback ran").unwrap();
    }
}
