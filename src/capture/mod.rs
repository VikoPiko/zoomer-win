#[cfg(windows)]
pub mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl Display {
    pub fn local_point(&self, x: i32, y: i32) -> (f64, f64) {
        (
            f64::from(x) - f64::from(self.x),
            f64::from(y) - f64::from(self.y),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra8,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("invalid or oversized screen dimensions/buffer (maximum 512 MiB)")]
    InvalidFrame,
    #[error("not enough memory for this monitor; close other applications and retry")]
    Allocation,
    #[error("desktop compositor synchronization failed ({0:#x}); unlock Windows and retry")]
    Composition(i32),
    #[error("{operation} failed: {source}")]
    Native {
        operation: &'static str,
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub struct Frame {
    pub display: Display,
    pub stride: usize,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
}
pub fn buffer_layout(width: u32, height: u32) -> Result<(usize, usize), CaptureError> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(CaptureError::InvalidFrame);
    }
    let stride = (width as usize)
        .checked_mul(4)
        .ok_or(CaptureError::InvalidFrame)?;
    let size = stride
        .checked_mul(height as usize)
        .filter(|n| *n <= 512 * 1024 * 1024)
        .ok_or(CaptureError::InvalidFrame)?;
    Ok((stride, size))
}
impl Frame {
    pub fn validate(&self) -> Result<(), CaptureError> {
        let (stride, size) = buffer_layout(self.display.width, self.display.height)?;
        if self.stride != stride || self.pixels.len() != size {
            return Err(CaptureError::InvalidFrame);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_buffers_and_negative_geometry() {
        assert_eq!(buffer_layout(3840, 2160).unwrap(), (15360, 33177600));
        for (w, h) in [(0, 1), (1, 0), (u32::MAX, 4), (32768, 32768)] {
            assert!(buffer_layout(w, h).is_err());
        }
        let display = Display {
            x: -2560,
            y: -100,
            width: 2560,
            height: 1440,
        };
        // Already physical coordinates: no second DPI scaling at 125% or 150%.
        assert_eq!(display.local_point(-2000, 200), (560.0, 300.0));
        let mut frame = Frame {
            display: Display {
                width: 1,
                height: 1,
                ..display
            },
            stride: 4,
            format: PixelFormat::Bgra8,
            pixels: vec![0; 4],
        };
        frame.validate().unwrap();
        frame.pixels.pop();
        assert!(frame.validate().is_err());
    }
}
