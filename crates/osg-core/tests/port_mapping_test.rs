use osg_core::pw::{Port, PortKind, map_ports};

fn ch5_1() -> (Vec<Port>, Vec<Port>) {
    let mut start = vec![
        Port::new_test(1, 0, PortKind::Source, false),
        Port::new_test(2, 0, PortKind::Source, false),
        Port::new_test(3, 0, PortKind::Source, false),
        Port::new_test(4, 0, PortKind::Source, false),
        Port::new_test(5, 0, PortKind::Source, false),
        Port::new_test(6, 0, PortKind::Source, false),
    ];
    start[0].channel = "FL".to_string();
    start[1].channel = "FR".to_string();
    start[2].channel = "RL".to_string();
    start[3].channel = "RR".to_string();
    start[4].channel = "FC".to_string();
    start[5].channel = "LFE".to_string();

    let mut end = vec![
        Port::new_test(7, 0, PortKind::Source, false),
        Port::new_test(8, 0, PortKind::Source, false),
        Port::new_test(9, 0, PortKind::Source, false),
        Port::new_test(10, 0, PortKind::Source, false),
        Port::new_test(11, 0, PortKind::Source, false),
        Port::new_test(12, 0, PortKind::Source, false),
    ];
    end[0].channel = "FL".to_string();
    end[1].channel = "FR".to_string();
    end[2].channel = "RL".to_string();
    end[3].channel = "RR".to_string();
    end[4].channel = "FC".to_string();
    end[5].channel = "LFE".to_string();

    (start, end)
}

#[test]
fn stereo_port_mapping() {
    let mut start = vec![
        Port::new_test(1, 0, PortKind::Source, false),
        Port::new_test(2, 0, PortKind::Source, false),
    ];
    start[0].channel = "FL".to_string();
    start[1].channel = "FR".to_string();

    let mut end = vec![
        Port::new_test(3, 0, PortKind::Source, false),
        Port::new_test(4, 0, PortKind::Source, false),
    ];
    end[0].channel = "FL".to_string();
    end[1].channel = "FR".to_string();

    let start_refs = start.iter().collect();
    let end_refs = end.iter().collect();

    assert_eq!(map_ports(start_refs, end_refs), vec![(1, 3), (2, 4)])
}

#[test]
fn ch5_1_port_mapping() {
    let (start, end) = ch5_1();

    let start_refs = start.iter().collect();
    let end_refs = end.iter().collect();

    assert_eq!(
        map_ports(start_refs, end_refs),
        vec![(1, 7), (2, 8), (3, 9), (4, 10), (5, 11), (6, 12)]
    )
}

#[test]
fn ch5_1_with_missing_port_in_end_mapping() {
    let (start, mut end) = ch5_1();

    end.remove(0);

    let start_refs = start.iter().collect();
    let end_refs = end.iter().collect();

    assert_eq!(
        map_ports(start_refs, end_refs),
        vec![(2, 8), (3, 9), (4, 10), (5, 11), (6, 12)]
    )
}

#[test]
fn mono_to_stereo_port_mapping() {
    let mut start = vec![Port::new_test(1, 0, PortKind::Source, false)];
    start[0].channel = "MONO".to_string();

    let mut end = vec![
        Port::new_test(2, 0, PortKind::Source, false),
        Port::new_test(3, 0, PortKind::Source, false),
    ];
    end[0].channel = "FL".to_string();
    end[1].channel = "FR".to_string();

    let start_refs = start.iter().collect();
    let end_refs = end.iter().collect();

    assert_eq!(map_ports(start_refs, end_refs), vec![(1, 2), (1, 3)])
}
