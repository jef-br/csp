//! Processor: executes the route the shot classifier chose — grow (crop + background stretch)
//! then final resize. Each route lives in its own file, named for what it does.

pub mod fill;
pub mod geometry;
pub mod resize;
