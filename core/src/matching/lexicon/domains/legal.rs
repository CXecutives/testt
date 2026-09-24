//! Legal, compliance and risk: compliance management, data protection (GDPR), anti money
//! laundering and financial crime, governance, risk management, internal controls, internal
//! audit, areas of law and in-house legal roles, information security.
//!
//! external contract - do not translate.

use super::Domain;

pub(crate) const DOMAIN: Domain = Domain {
    name: "legal",
    triggers: &[
        "aml",
        "compliance",
        "datenschutz",
        "dsgvo",
        "exportkontroll",
        "gdpr",
        "geldwasche",
        "governance",
        "grc",
        "hinweisgeber",
        "iks",
        "informationssicherheit",
        "jurist",
        "justiziar",
        "korruption",
        "kyc",
        "legal",
        "rechtsabteil",
        "rechtsanwalt",
        "revision",
        "risikomanagement",
        "risk",
        "sanktion",
        "sox",
        "syndikus",
        "vertragsrecht",
        "whistleblow",
    ],
    generic: &["governance", "recht"],
    concepts: &[
        // Compliance.
        ("compliance officer", "compliance"),
        ("compliance beauftragter", "compliance"),
        ("compliance manager", "compliance"),
        ("head compliance", "compliance-leitung"),
        ("leiter compliance", "compliance-leitung"),
        ("leiterin compliance", "compliance-leitung"),
        ("leitung compliance", "compliance-leitung"),
        ("chief compliance officer", "compliance-leitung"),
        ("governance risk compliance", "grc"),
        ("whistleblowing", "hinweisgebersystem"),
        ("whistleblower system", "hinweisgebersystem"),
        ("hinschg", "hinweisgebersystem"),
        ("anti corruption", "korruptionspravention"),
        ("anti bribery", "korruptionspravention"),
        ("korruptionsbekampfung", "korruptionspravention"),
        ("abac", "korruptionspravention"),
        ("export control", "exportkontrolle"),
        ("trade compliance", "exportkontrolle"),
        ("aussenwirtschaftsrecht", "exportkontrolle"),
        ("lksg", "lieferkettengesetz"),
        ("supply chain act", "lieferkettengesetz"),
        // Data protection.
        ("data protection", "datenschutz"),
        ("data privacy", "datenschutz"),
        ("privacy", "datenschutz"),
        ("data protection regulation", "datenschutz"),
        ("datenschutz grundverordnung", "datenschutz"),
        ("dsgvo", "datenschutz"),
        ("eu-dsgvo", "datenschutz"),
        ("gdpr", "datenschutz"),
        ("data protection officer", "datenschutzbeauftragter"),
        ("dpo", "datenschutzbeauftragter"),
        // Anti money laundering and financial crime.
        ("anti money laundering", "geldwaschepravention"),
        ("money laundering", "geldwaschepravention"),
        ("aml", "geldwaschepravention"),
        ("geldwasche", "geldwaschepravention"),
        ("anti geldwasche", "geldwaschepravention"),
        ("geldwaschebekampfung", "geldwaschepravention"),
        ("geldwaschegesetz", "gwg"),
        ("aml officer", "geldwaschebeauftragter"),
        ("mlro", "geldwaschebeauftragter"),
        ("know customer", "kyc"),
        ("kundensorgfaltspflichten", "kyc"),
        ("customer due diligence", "kyc"),
        ("cdd", "kyc"),
        ("sanctions", "sanktion"),
        ("sanctions screening", "sanktionsscreening"),
        ("financial crime", "finanzkriminalitat"),
        ("fraud", "betrugspravention"),
        ("fraud prevention", "betrugspravention"),
        ("betrugsbekampfung", "betrugspravention"),
        // Governance, risk and internal controls.
        ("corporate governance", "corporate-governance"),
        ("risk management", "risikomanagement"),
        ("enterprise risk management", "risikomanagement"),
        ("erm", "risikomanagement"),
        ("risikomanagementsystem", "risikomanagement"),
        ("chief risk officer", "risikomanagement"),
        ("operational risk", "operationellesrisiko"),
        ("operationelle risiken", "operationellesrisiko"),
        ("risk manager", "risikomanagement"),
        ("risk assessment", "risikobewertung"),
        ("risikoanalyse", "risikobewertung"),
        ("internal control system", "iks"),
        ("internal controls", "iks"),
        ("internes kontrollsystem", "iks"),
        ("interne kontrollen", "iks"),
        ("ics", "iks"),
        ("icfr", "iks"),
        ("sox", "iks"),
        ("sarbanes oxley", "iks"),
        ("internal audit", "revision"),
        ("internal auditor", "revision"),
        ("interne revision", "revision"),
        ("innenrevision", "revision"),
        ("external audit", "wirtschaftsprufung"),
        ("regulatory reporting", "meldewesen"),
        ("regulatory law", "aufsichtsrecht"),
        ("bankaufsichtsrecht", "aufsichtsrecht"),
        ("bankenaufsichtsrecht", "aufsichtsrecht"),
        // Areas of law and legal work.
        ("contract law", "vertragsrecht"),
        ("contract management", "vertragsmanagement"),
        ("contract drafting", "vertragsgestaltung"),
        ("contract review", "vertragsprufung"),
        ("employment law", "arbeitsrecht"),
        ("labour law", "arbeitsrecht"),
        ("labor law", "arbeitsrecht"),
        ("corporate law", "gesellschaftsrecht"),
        ("company law", "gesellschaftsrecht"),
        ("commercial law", "handelsrecht"),
        ("competition law", "kartellrecht"),
        ("antitrust", "kartellrecht"),
        ("antitrust law", "kartellrecht"),
        ("it-law", "it-recht"),
        ("public procurement law", "vergaberecht"),
        ("tax law", "steuerrecht"),
        ("insolvency law", "insolvenzrecht"),
        ("legal advice", "rechtsberatung"),
        ("litigation", "prozessfuhrung"),
        ("dispute resolution", "streitbeilegung"),
        // In-house legal roles (`General` is a filler, so `General Counsel` is `Counsel`).
        ("legal counsel", "syndikus"),
        ("general counsel", "leitung-recht"),
        ("in-house counsel", "syndikus"),
        ("syndikusrechtsanwalt", "syndikus"),
        ("syndikusrechtsanwaltin", "syndikus"),
        ("syndikusanwalt", "syndikus"),
        ("syndikusanwaltin", "syndikus"),
        ("unternehmensjurist", "syndikus"),
        ("unternehmensjuristin", "syndikus"),
        ("justiziar", "syndikus"),
        ("justiziarin", "syndikus"),
        ("counsel", "leitung-recht"),
        ("head legal", "leitung-recht"),
        ("leiter recht", "leitung-recht"),
        ("leiterin recht", "leitung-recht"),
        ("leitung recht", "leitung-recht"),
        ("leiter rechtsabteilung", "leitung-recht"),
        ("leiterin rechtsabteilung", "leitung-recht"),
        ("leitung rechtsabteilung", "leitung-recht"),
        ("chefjustiziar", "leitung-recht"),
        ("chefjustiziarin", "leitung-recht"),
        ("juristin", "jurist"),
        // Information security.
        ("information security", "informationssicherheit"),
        ("isms", "informationssicherheit"),
        ("it-security", "it-sicherheit"),
        ("cyber security", "it-sicherheit"),
        ("cybersecurity", "it-sicherheit"),
        // False friends: audit-proof archiving is no internal audit.
        ("revisionssicher", "archivierung"),
        ("revisionssicherheit", "archivierung"),
    ],
};

#[cfg(test)]
mod tests {
    use super::super::testing::{assert_apart, assert_same, vocab};
    use crate::matching::atoms::{Vocab, is_generic};

    fn legal() -> Vocab {
        let v = vocab(&["Compliance", "Datenschutz", "Geldwäscheprävention"]);
        assert_eq!(v.packs(), ["legal"]);
        v
    }

    #[test]
    fn paraphrases() {
        assert_same(
            &legal(),
            &[
                ("Compliance Officer", "Compliance"),
                ("Head of Compliance", "Leiterin Compliance"),
                ("Data Protection", "Datenschutz"),
                ("GDPR", "DSGVO"),
                ("EU-DSGVO", "Datenschutz-Grundverordnung"),
                ("General Data Protection Regulation", "Datenschutz"),
                ("Anti Money Laundering", "Geldwäscheprävention"),
                ("AML", "Geldwäsche"),
                ("Know Your Customer", "KYC"),
                ("Risk Management", "Risikomanagement"),
                ("Internal Control System", "IKS"),
                ("Internes Kontrollsystem", "Internal Controls"),
                ("SOX", "IKS"),
                ("Internal Audit", "Interne Revision"),
                ("Innenrevision", "Revision"),
                ("Legal Counsel", "Syndikusrechtsanwältin"),
                ("General Counsel", "Leiter Rechtsabteilung"),
                ("Contract law", "Vertragsrecht"),
                ("Corporate law", "Gesellschaftsrecht"),
                ("Antitrust", "Kartellrecht"),
                ("Whistleblowing", "Hinweisgebersystem"),
                ("Export Control", "Exportkontrolle"),
                ("Information Security", "Informationssicherheit"),
                ("Juristin", "Jurist"),
            ],
        );
    }

    #[test]
    fn false_friends_stay_apart() {
        assert_apart(
            &legal(),
            &[
                ("Revisionssicherheit", "Interne Revision"),
                ("revisionssichere Archivierung", "Revision"),
                ("Arbeitsschutz", "Datenschutz"),
                ("AMG", "AML"),
                ("Data Governance", "Corporate Governance"),
            ],
        );
        for word in ["recht", "governance", "governanc"] {
            assert!(is_generic(word), "{word}");
        }
    }
}
