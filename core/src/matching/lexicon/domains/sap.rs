//! SAP and ERP: products, modules and process names.
//!
//! external contract - do not translate.

use super::Domain;

pub(crate) const DOMAIN: Domain = Domain {
    name: "sap",
    triggers: &["abap", "fiori", "s/4hana", "s4hana", "sap"],
    generic: &[],
    concepts: &[
        ("order to cash", "order-to-cash"),
        ("otc", "order-to-cash"),
        ("procure to pay", "procure-to-pay"),
        ("s4hana", "s/4hana"),
        ("s4", "s/4hana"),
        ("central finance", "central-finance"),
        ("fi-gl", "hauptbuchhaltung"),
        ("fi-ap", "kreditorenbuchhaltung"),
        ("fi-ar", "debitorenbuchhaltung"),
        ("fi-aa", "anlagenbuchhaltung"),
        ("co-om", "kostenstellenrechnung"),
        ("co-pc", "produktkostenrechnung"),
        ("co-pa", "ergebnisrechnung"),
    ],
};
