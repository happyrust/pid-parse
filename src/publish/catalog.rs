//! Single source of truth for publish item loading and XML emission.

use std::sync::OnceLock;

/// Canonical publish item families understood by the loader and writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PublishItemKind {
    Vessel,
    Nozzle,
    PipeRun,
    PipingPoint,
    PipingComponent,
    Instrument,
    Note,
    Exchanger,
    Mechanical,
    SignalRun,
    BranchPoint,
    PipingBranchPoint,
}

/// Ordered XML emission actions for one publish item.
///
/// Actions, rather than writer function pointers, model the real 1:N
/// relationship between one source item and its generated PID tags while
/// allowing the writer to keep family-specific function signatures private.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PublishEmission {
    ProcessVessel,
    Nozzle,
    Pipeline,
    PipingConnector,
    DerivedConnectorEndpoints,
    PipingPort,
    Note,
    ControlSystemFunction,
    DerivedSignalPorts,
    PipingComponent,
    SignalConnector,
    BranchPoint,
    PipingBranchPoint,
    GenericPlaceholder,
}

impl PublishEmission {
    fn pid_tags(self) -> &'static [&'static str] {
        match self {
            Self::ProcessVessel => &["PIDProcessVessel"],
            Self::Nozzle => &["PIDNozzle"],
            Self::Pipeline => &["PIDPipeline"],
            Self::PipingConnector => &["PIDPipingConnector"],
            Self::DerivedConnectorEndpoints => &["PIDPipingPort", "PIDProcessPoint"],
            Self::PipingPort => &["PIDPipingPort"],
            Self::Note => &["PIDNote"],
            Self::ControlSystemFunction => &["PIDControlSystemFunction"],
            Self::DerivedSignalPorts => &["PIDSignalPort"],
            Self::PipingComponent => &["PIDPipingComponent"],
            Self::SignalConnector => &["PIDSignalConnector"],
            Self::BranchPoint => &["PIDBranchPoint"],
            Self::PipingBranchPoint => &["PIDPipingBranchPoint"],
            // `<PIDItem>` is a conservative fallback, not a canonical
            // SmartPlant tag whose shape the writer claims to support.
            Self::GenericPlaceholder => &[],
        }
    }
}

/// Cross-cutting facts for one canonical publish item family.
#[derive(Debug)]
pub(super) struct PublishItemSpec {
    pub kind: PublishItemKind,
    pub aliases: &'static [&'static str],
    pub subtables: &'static [&'static str],
    pub a01_rank: u8,
    pub emissions: &'static [PublishEmission],
}

const ITEM_SPECS: &[PublishItemSpec] = &[
    PublishItemSpec {
        kind: PublishItemKind::Vessel,
        aliases: &["Vessel"],
        subtables: &[
            "T_PlantItem",
            "T_Equipment",
            "T_ProcessEquipment",
            "T_Vessel",
        ],
        a01_rank: 0,
        emissions: &[PublishEmission::ProcessVessel],
    },
    PublishItemSpec {
        kind: PublishItemKind::Nozzle,
        aliases: &["Nozzle"],
        subtables: &["T_PlantItem", "T_EquipComponent", "T_Nozzle"],
        a01_rank: 1,
        emissions: &[PublishEmission::Nozzle],
    },
    PublishItemSpec {
        kind: PublishItemKind::PipeRun,
        aliases: &["PipeRun"],
        subtables: &["T_PlantItem", "T_Connector", "T_PipeRun", "T_Pipeline"],
        a01_rank: 2,
        emissions: &[
            PublishEmission::Pipeline,
            PublishEmission::PipingConnector,
            PublishEmission::DerivedConnectorEndpoints,
        ],
    },
    PublishItemSpec {
        kind: PublishItemKind::PipingPoint,
        aliases: &["PipingPoint"],
        subtables: &["T_PipingPoint"],
        a01_rank: 9,
        emissions: &[PublishEmission::PipingPort],
    },
    PublishItemSpec {
        kind: PublishItemKind::PipingComponent,
        aliases: &["PipingComp"],
        subtables: &["T_PlantItem", "T_InlineComp", "T_PipingComp"],
        a01_rank: 9,
        emissions: &[PublishEmission::PipingComponent],
    },
    PublishItemSpec {
        kind: PublishItemKind::Instrument,
        aliases: &["Instrument", "InstrFunction"],
        subtables: &["T_PlantItem", "T_Instrument", "T_InstrFunction"],
        a01_rank: 9,
        emissions: &[
            PublishEmission::ControlSystemFunction,
            PublishEmission::DerivedSignalPorts,
        ],
    },
    PublishItemSpec {
        kind: PublishItemKind::Note,
        aliases: &["Note", "ItemNote"],
        subtables: &["T_ItemNote"],
        a01_rank: 9,
        emissions: &[PublishEmission::Note],
    },
    PublishItemSpec {
        kind: PublishItemKind::Exchanger,
        aliases: &["Exchanger"],
        subtables: &["T_PlantItem", "T_Equipment", "T_Exchanger"],
        a01_rank: 9,
        emissions: &[PublishEmission::GenericPlaceholder],
    },
    PublishItemSpec {
        kind: PublishItemKind::Mechanical,
        aliases: &["Mechanical"],
        subtables: &["T_PlantItem", "T_Equipment", "T_Mechanical"],
        a01_rank: 9,
        emissions: &[PublishEmission::GenericPlaceholder],
    },
    PublishItemSpec {
        kind: PublishItemKind::SignalRun,
        aliases: &["SignalRun"],
        subtables: &["T_PlantItem", "T_Connector", "T_SignalRun"],
        a01_rank: 9,
        emissions: &[PublishEmission::SignalConnector],
    },
    PublishItemSpec {
        kind: PublishItemKind::BranchPoint,
        aliases: &["BranchPoint"],
        subtables: &["T_PlantItem"],
        a01_rank: 9,
        emissions: &[PublishEmission::BranchPoint],
    },
    PublishItemSpec {
        kind: PublishItemKind::PipingBranchPoint,
        aliases: &["PipingBranchPoint"],
        subtables: &["T_PlantItem"],
        a01_rank: 9,
        emissions: &[PublishEmission::PipingBranchPoint],
    },
];

const DOCUMENT_PID_TAGS: &[&str] = &["PIDDrawing", "PIDRepresentation"];

/// MDF tables required by the publish staging adapter.
pub(super) const PUBLISH_STAGING_TABLES: &[&str] = &[
    "T_Drawing",
    "T_Representation",
    "T_Relationship",
    "T_ModelItem",
    "T_PipingPoint",
    "T_PlantItem",
    "T_Equipment",
    "T_ProcessEquipment",
    "T_Vessel",
    "T_EquipComponent",
    "T_Nozzle",
    "T_Connector",
    "T_PipeRun",
    "T_Pipeline",
    "T_InlineComp",
    "T_PipingComp",
    "T_Instrument",
    "T_InstrFunction",
    "T_ItemNote",
    "T_Exchanger",
    "T_Mechanical",
    "T_SignalRun",
    "codelists",
    "attributes",
];

pub(super) fn item_spec(item_type_name: &str) -> Option<&'static PublishItemSpec> {
    ITEM_SPECS
        .iter()
        .find(|spec| spec.aliases.contains(&item_type_name))
}

pub(super) fn subtables_for_item_type(item_type_name: &str) -> &'static [&'static str] {
    item_spec(item_type_name).map_or(&[], |spec| spec.subtables)
}

pub(super) fn a01_rank(item_type_name: &str) -> u8 {
    item_spec(item_type_name).map_or(9, |spec| spec.a01_rank)
}

pub(super) fn supported_pid_tags() -> &'static [&'static str] {
    static TAGS: OnceLock<Vec<&'static str>> = OnceLock::new();
    TAGS.get_or_init(|| {
        let mut tags = DOCUMENT_PID_TAGS.to_vec();
        for spec in ITEM_SPECS {
            for emission in spec.emissions {
                tags.extend_from_slice(emission.pid_tags());
            }
        }
        tags.sort_unstable();
        tags.dedup();
        tags
    })
    .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_run_catalog_entry_carries_loading_order_and_one_to_many_emissions() {
        let spec = item_spec("PipeRun").expect("PipeRun catalog entry");

        assert_eq!(spec.kind, PublishItemKind::PipeRun);
        assert_eq!(
            spec.subtables,
            &["T_PlantItem", "T_Connector", "T_PipeRun", "T_Pipeline"]
        );
        assert_eq!(spec.a01_rank, 2);
        assert_eq!(
            spec.emissions,
            &[
                PublishEmission::Pipeline,
                PublishEmission::PipingConnector,
                PublishEmission::DerivedConnectorEndpoints,
            ]
        );
    }

    #[test]
    fn aliases_resolve_to_one_item_kind() {
        assert_eq!(
            item_spec("Instrument").map(|spec| spec.kind),
            Some(PublishItemKind::Instrument)
        );
        assert_eq!(
            item_spec("InstrFunction").map(|spec| spec.kind),
            Some(PublishItemKind::Instrument)
        );
        assert_eq!(
            item_spec("Note").map(|spec| spec.kind),
            Some(PublishItemKind::Note)
        );
        assert_eq!(
            item_spec("ItemNote").map(|spec| spec.kind),
            Some(PublishItemKind::Note)
        );
        assert!(item_spec("UnregisteredType").is_none());
    }

    #[test]
    fn supported_tags_are_derived_sorted_and_unique() {
        assert_eq!(
            supported_pid_tags(),
            &[
                "PIDBranchPoint",
                "PIDControlSystemFunction",
                "PIDDrawing",
                "PIDNote",
                "PIDNozzle",
                "PIDPipeline",
                "PIDPipingBranchPoint",
                "PIDPipingComponent",
                "PIDPipingConnector",
                "PIDPipingPort",
                "PIDProcessPoint",
                "PIDProcessVessel",
                "PIDRepresentation",
                "PIDSignalConnector",
                "PIDSignalPort",
            ]
        );
    }

    #[test]
    fn staging_tables_cover_the_current_mdf_adapter_contract() {
        assert_eq!(
            PUBLISH_STAGING_TABLES,
            &[
                "T_Drawing",
                "T_Representation",
                "T_Relationship",
                "T_ModelItem",
                "T_PipingPoint",
                "T_PlantItem",
                "T_Equipment",
                "T_ProcessEquipment",
                "T_Vessel",
                "T_EquipComponent",
                "T_Nozzle",
                "T_Connector",
                "T_PipeRun",
                "T_Pipeline",
                "T_InlineComp",
                "T_PipingComp",
                "T_Instrument",
                "T_InstrFunction",
                "T_ItemNote",
                "T_Exchanger",
                "T_Mechanical",
                "T_SignalRun",
                "codelists",
                "attributes",
            ]
        );
    }
}
