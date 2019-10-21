//! Representation of colors.

/// Color represented with red, green, blue and alpha components.
#[derive(Clone, Debug)]
pub struct Color([f32; 4]);

impl Color {
    /// Creates a black color.
    pub fn black() -> Self {
        Self([0.0, 0.0, 0.0, 1.0])
    }

    /// Returns a 4-element slice containing the red, green, blue and alpha values of
    /// the color.
    pub fn to_slice(&self) -> [f32; 4] {
        let color = self.clone();
        color.0
    }

    /// Consumes the color and returns the 4-element slice containing the red, green,
    /// blue and alpha values of the color.
    pub fn into_slice(self) -> [f32; 4] {
        self.0
    }
}
