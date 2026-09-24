//! Sales and marketing: sales leadership, key account management, business development,
//! new business, CRM tools, marketing (performance, online, brand),
//! pricing and sales channels.
//!
//! external contract - do not translate.

use super::Domain;

pub(crate) const DOMAIN: Domain = Domain {
    name: "sales",
    triggers: &[
        "crm",
        "e-commerce",
        "ecommerce",
        "hubspot",
        "kundenakquis",
        "markenfuhr",
        "marketing",
        "neukunden",
        "preismanagement",
        "preisstrateg",
        "pricing",
        "sales",
        "seo",
        "vertrieb",
    ],
    generic: &["client", "customer", "kund"],
    concepts: &[
        // Sales and its leadership.
        ("sales", "vertrieb"),
        ("verkauf", "vertrieb"),
        ("vertrieblich", "vertrieb"),
        ("vertriebsleiter", "vertriebsleitung"),
        ("vertriebsleiterin", "vertriebsleitung"),
        ("leiter vertrieb", "vertriebsleitung"),
        ("leiterin vertrieb", "vertriebsleitung"),
        ("leitung vertrieb", "vertriebsleitung"),
        ("head sales", "vertriebsleitung"),
        ("sales director", "vertriebsleitung"),
        ("director sales", "vertriebsleitung"),
        ("chief sales officer", "vertriebsleitung"),
        ("vertriebsdirektor", "vertriebsleitung"),
        ("vertriebsdirektorin", "vertriebsleitung"),
        ("verkaufsleiter", "vertriebsleitung"),
        ("verkaufsleiterin", "vertriebsleitung"),
        ("verkaufsleitung", "vertriebsleitung"),
        ("sales excellence", "sales-excellence"),
        ("sales steering", "vertriebssteuerung"),
        ("sales controlling", "vertriebscontrolling"),
        ("inside sales", "innendienst"),
        ("vertriebsinnendienst", "innendienst"),
        ("sales support", "innendienst"),
        ("field sales", "aussendienst"),
        ("vertriebsaussendienst", "aussendienst"),
        ("channel management", "channel-management"),
        ("indirect sales", "indirektervertrieb"),
        ("indirekter vertrieb", "indirektervertrieb"),
        ("channel sales", "indirektervertrieb"),
        ("partnervertrieb", "indirektervertrieb"),
        ("partner management", "partnermanagement"),
        ("bid management", "angebotsmanagement"),
        ("proposal management", "angebotsmanagement"),
        ("tender management", "angebotsmanagement"),
        ("angebotserstellung", "angebotsmanagement"),
        // Accounts and customers (`Accounting` is bookkeeping, not accounts).
        ("key account management", "key-account-management"),
        ("key account manager", "key-account-management"),
        ("key account", "key-account-management"),
        ("kam", "key-account-management"),
        ("grosskundenbetreuung", "key-account-management"),
        ("grosskundenvertrieb", "key-account-management"),
        ("account management", "kundenbetreuung"),
        ("account manager", "kundenbetreuung"),
        ("bestandskundenbetreuung", "kundenbetreuung"),
        ("bestandskundenmanagement", "kundenbetreuung"),
        ("accounting", "rechnungswesen"),
        ("customer success", "customer-success"),
        ("customer service", "kundenservice"),
        // Business development and new business.
        ("business development", "business-development"),
        ("business developer", "business-development"),
        ("bizdev", "business-development"),
        ("geschaftsentwicklung", "business-development"),
        ("geschaftsfeldentwicklung", "business-development"),
        ("neukundenakquise", "neukundengewinnung"),
        ("kundenakquise", "neukundengewinnung"),
        ("akquise", "neukundengewinnung"),
        ("customer acquisition", "neukundengewinnung"),
        ("lead generation", "leadgenerierung"),
        ("cold calling", "kaltakquise"),
        // CRM.
        ("customer relationship management", "crm"),
        ("kundenbeziehungsmanagement", "crm"),
        ("sfdc", "salesforce"),
        ("salesforce.com", "salesforce"),
        // Marketing.
        ("performance marketing", "performancemarketing"),
        ("online marketing", "onlinemarketing"),
        ("digital marketing", "onlinemarketing"),
        ("search engine advertising", "sea"),
        ("suchmaschinenwerbung", "sea"),
        ("google ads", "sea"),
        ("adwords", "sea"),
        ("search engine optimization", "seo"),
        ("search engine optimisation", "seo"),
        ("suchmaschinenoptimierung", "seo"),
        ("content marketing", "contentmarketing"),
        ("social media marketing", "socialmediamarketing"),
        ("marketing automation", "marketingautomation"),
        ("product marketing", "produktmarketing"),
        ("product management", "produktmanagement"),
        ("produktmanager", "produktmanagement"),
        ("produktmanagerin", "produktmanagement"),
        ("brand management", "markenfuhrung"),
        ("markenmanagement", "markenfuhrung"),
        ("branding", "markenfuhrung"),
        ("marketingleiter", "marketingleitung"),
        ("marketingleiterin", "marketingleitung"),
        ("leiter marketing", "marketingleitung"),
        ("leiterin marketing", "marketingleitung"),
        ("leitung marketing", "marketingleitung"),
        ("head marketing", "marketingleitung"),
        ("marketing director", "marketingleitung"),
        ("director marketing", "marketingleitung"),
        ("chief marketing officer", "marketingleitung"),
        ("cmo", "marketingleitung"),
        ("campaign management", "kampagnenmanagement"),
        ("market research", "marktforschung"),
        ("ecommerce", "e-commerce"),
        ("onlinehandel", "e-commerce"),
        ("webshop", "e-commerce"),
        // Pricing (transfer prices and revenue recognition belong to finance).
        ("pricing", "preismanagement"),
        ("price management", "preismanagement"),
        ("pricing strategy", "preismanagement"),
        ("preisgestaltung", "preismanagement"),
        ("preispolitik", "preismanagement"),
        ("preisstrategie", "preismanagement"),
        ("preisfindung", "preismanagement"),
        ("revenue management", "revenue-management"),
        ("transfer pricing", "verrechnungspreis"),
        ("revenue recognition", "umsatzrealisierung"),
        ("negotiation", "verhandlung"),
        ("sales operations planning", "s&op"),
    ],
};

#[cfg(test)]
mod tests {
    use super::super::testing::{assert_apart, assert_same, vocab};
    use crate::matching::atoms::Vocab;

    fn sales() -> Vocab {
        let v = vocab(&[
            "Vertriebsleitung",
            "Key Account Management",
            "CRM",
            "Pricing",
        ]);
        assert_eq!(v.packs(), ["sales"]);
        v
    }

    #[test]
    fn paraphrases() {
        assert_same(
            &sales(),
            &[
                ("Sales", "Vertrieb"),
                ("Head of Sales", "Vertriebsleiterin"),
                ("Sales Director", "Leitung Vertrieb"),
                ("Key Account Manager", "Key Account Management"),
                ("Key Accounts", "KAM"),
                ("Großkundenbetreuung", "Key-Account-Management"),
                ("Business Development", "Geschäftsentwicklung"),
                ("Neukundenakquise", "Akquise"),
                ("Customer Relationship Management", "CRM"),
                ("SFDC", "Salesforce"),
                ("Online Marketing", "Digital Marketing"),
                ("Suchmaschinenoptimierung", "SEO"),
                ("Google Ads", "SEA"),
                ("Head of Marketing", "Marketingleiterin"),
                ("Brand Management", "Markenführung"),
                ("Pricing", "Preisgestaltung"),
                ("Pricing strategy", "Preisstrategie"),
                ("Inside Sales", "Vertriebsinnendienst"),
                ("Indirect sales", "Partnervertrieb"),
                ("eCommerce", "E-Commerce"),
            ],
        );
    }

    #[test]
    fn false_friends_stay_apart() {
        assert_apart(
            &sales(),
            &[
                ("Accounting", "Account Management"),
                ("Accounts Payable", "Key Account Management"),
                ("Transfer Pricing", "Pricing"),
                ("Revenue Recognition", "Revenue Management"),
                ("Akquisition", "Akquise"),
                ("Vertriebscontrolling", "Vertrieb"),
                ("Performance Management", "Performance Marketing"),
            ],
        );
    }
}
