//! Electrical parameterisation: cited line and transformer class tables,
//! population weighted loads, reserve-margin generation, branch impedances
//! with their ratings, the corridor plan that sizes circuits to the planning
//! dispatch, and the reactive plan that balances every bus.

pub mod branches;
pub mod generators;
pub mod loads;
pub mod params;
pub mod planning;
pub mod reactive;
