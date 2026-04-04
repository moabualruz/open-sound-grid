// Pure port-mapping algorithm — no PipeWire state, no side effects.
//
// Determines how to connect two lists of ports by channel name, with fallbacks
// for mono-to-multi and positional mapping when names don't align.

use super::object::Port;

/// Maps two lists of ports to (source_id, sink_id) pairs.
///
/// | Situation        | Behaviour                               |
/// |------------------|-----------------------------------------|
/// | start.len() == 1 | single source → every end port          |
/// | otherwise        | match by channel name, then by position |
pub fn map_ports<P>(start: Vec<&Port<P>>, end: Vec<&Port<P>>) -> Vec<(u32, u32)> {
    if start.len() == 1 {
        return end
            .iter()
            .map(|end_port| (start[0].id, end_port.id))
            .collect();
    }
    let pairs: Vec<(u32, u32)> = start
        .iter()
        .enumerate()
        .filter_map(|(index, start_port)| {
            let start_port_id: u32 = start_port.id;
            let end_port_id: Option<u32> = end
                .get(index)
                .and_then(|port| (port.channel == start_port.channel).then_some(port.id))
                .or_else(|| {
                    Some(
                        end.iter()
                            .find(|end_port| end_port.channel == start_port.channel)?
                            .id,
                    )
                });
            if end_port_id.is_none() {
                tracing::trace!("Could not find matching end port for {}", start_port_id);
            }
            Some((start_port_id, end_port_id?))
        })
        .collect();
    // Fall back to positional mapping when channel names don't match
    if pairs.is_empty() && !start.is_empty() && !end.is_empty() {
        return start
            .iter()
            .zip(end.iter())
            .map(|(s, e)| (s.id, e.id))
            .collect();
    }
    pairs
}
