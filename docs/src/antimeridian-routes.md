//! Geographic route coordinates are serialized in [-180, 180]. Map consumers
//! should draw the shortest longitude hop between adjacent positions across the
//! antimeridian; otherwise a route near ±180 can appear to span the whole world.
