use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price {
    pub paisa: u64,
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        
        let rupees: u64 = self.paisa / 100;
        let paisa: u64 = self.paisa % 100;
        
        write!(f, "₹{}.{:02}", rupees, paisa)
    }
}


impl Price {
    pub fn from_paisa(paisa: u64) -> Self {
        Self { paisa }
    }
    pub fn from_rupees(rupees: f64) -> Self {
        Self { paisa: (rupees * 100.0).round() as u64 }
    }
    pub fn from_rupees_paisa(rupees: u64, paisa: u64) -> Self {
        Self { paisa: (rupees * 100) + paisa }
    }
    pub fn get_price(&self) -> f64 {
        (self.paisa / 100) as f64 + ((self.paisa % 100) as f64) / 100.0
    }
}