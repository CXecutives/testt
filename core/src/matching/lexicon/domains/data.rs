//! Data and business intelligence: BI tools, SQL, data warehouses and modelling, ETL/ELT,
//! data platforms, governance, analytics and data science, reporting and KPIs.
//!
//! external contract - do not translate.

use super::Domain;

pub(crate) const DOMAIN: Domain = Domain {
    name: "data",
    triggers: &[
        "airflow",
        "bigquery",
        "dashboard",
        "data",
        "datenanalys",
        "datenarchitekt",
        "datenbank",
        "datenintegration",
        "datenmodell",
        "datenplattform",
        "datenqualitat",
        "datenvisualis",
        "dax",
        "dbt",
        "dwh",
        "etl",
        "hadoop",
        "informatica",
        "intelligence",
        "looker",
        "machine",
        "olap",
        "pandas",
        "power-bi",
        "powerbi",
        "python",
        "qlik",
        "redshift",
        "snowflake",
        "sql",
        "ssis",
        "statist",
        "synapse",
        "tableau",
        "talend",
    ],
    generic: &[],
    concepts: &[
        // The field, its roles and disciplines.
        ("business intelligence", "business-intelligence"),
        ("bi", "business-intelligence"),
        ("bi consultant", "business-intelligence"),
        ("bi berater", "business-intelligence"),
        ("bi beraterin", "business-intelligence"),
        ("bi developer", "business-intelligence"),
        ("bi entwickler", "business-intelligence"),
        ("bi entwicklerin", "business-intelligence"),
        ("bi analyst", "business-intelligence"),
        ("data analytics", "datenanalyse"),
        ("data analysis", "datenanalyse"),
        ("data analyst", "datenanalyse"),
        ("datenauswertung", "datenanalyse"),
        ("data engineering", "data-engineering"),
        ("data engineer", "data-engineering"),
        ("data science", "data-science"),
        ("data scientist", "data-science"),
        ("machine learning", "ml"),
        ("maschinelles lernen", "ml"),
        ("kunstliche intelligenz", "ki"),
        ("artificial intelligence", "ki"),
        ("statistics", "statistik"),
        ("statistical analysis", "statistik"),
        ("predictive analytics", "predictive-analytics"),
        // Tools and languages.
        ("power bi", "powerbi"),
        ("power query", "powerquery"),
        ("power pivot", "powerpivot"),
        ("qlikview", "qlik"),
        ("qliksense", "qlik"),
        ("qlik sense", "qlik"),
        ("azure data factory", "adf"),
        ("data factory", "adf"),
        ("azure synapse", "synapse"),
        ("synapse analytics", "synapse"),
        ("azure synapse analytics", "synapse"),
        ("data build tool", "dbt"),
        ("structured query language", "sql"),
        ("tsql", "sql"),
        ("t-sql", "sql"),
        ("sql server integration services", "ssis"),
        ("sql server reporting services", "ssrs"),
        ("sql server analysis services", "ssas"),
        ("python3", "python"),
        // Warehouses, platforms and modelling.
        ("data warehouse", "data-warehouse"),
        ("datawarehouse", "data-warehouse"),
        ("dwh", "data-warehouse"),
        ("data lake", "data-lake"),
        ("data lakehouse", "lakehouse"),
        ("data vault", "data-vault"),
        ("data mart", "data-mart"),
        ("data modelling", "datenmodellierung"),
        ("data model", "datenmodellierung"),
        ("datenmodell", "datenmodellierung"),
        ("dimensional modelling", "datenmodellierung"),
        ("dimensional modeling", "datenmodellierung"),
        ("star schema", "datenmodellierung"),
        ("sternschema", "datenmodellierung"),
        ("snowflake schema", "datenmodellierung"),
        ("elt", "etl"),
        ("extract transform load", "etl"),
        ("etl strecken", "etl"),
        ("etl prozesse", "etl"),
        ("etl pipelines", "etl"),
        ("data pipeline", "etl"),
        ("datenpipeline", "etl"),
        ("data integration", "datenintegration"),
        ("database", "datenbank"),
        // Governance and quality.
        ("data governance", "data-governance"),
        ("datengovernance", "data-governance"),
        ("data quality", "datenqualitat"),
        ("datenqualitatsmanagement", "datenqualitat"),
        ("master data management", "stammdatenmanagement"),
        ("mdm", "stammdatenmanagement"),
        ("master data", "stammdat"),
        // Reporting and KPIs.
        ("berichtswesen", "reporting"),
        ("data visualization", "datenvisualisierung"),
        ("data visualisation", "datenvisualisierung"),
        ("visualisierung", "datenvisualisierung"),
        ("visualization", "datenvisualisierung"),
        ("visualisation", "datenvisualisierung"),
        ("kpis", "kpi"),
        ("kennzahl", "kpi"),
        ("kennzahlensystem", "kpi"),
        ("key performance indicator", "kpi"),
        // False friends: the stock index, a savings bank, excellence and warehouses of
        // goods.
        ("dax konzern", "borsennotiert"),
        ("dax unternehmen", "borsennotiert"),
        ("dax notiert", "borsennotiert"),
        ("sparkasse", "kreditinstitut"),
        ("excellence", "exzellenz"),
        ("warehouse management", "lagerlogistik"),
        ("warehouse logistics", "lagerlogistik"),
    ],
};

#[cfg(test)]
mod tests {
    use super::super::testing::{assert_apart, assert_same, vocab};
    use crate::matching::atoms::Vocab;

    fn data() -> Vocab {
        let v = vocab(&["Power BI", "SQL", "Datenmodellierung"]);
        assert_eq!(v.packs(), ["data"]);
        v
    }

    #[test]
    fn paraphrases() {
        assert_same(
            &data(),
            &[
                ("Power BI", "PowerBI"),
                ("Power-BI", "Power BI"),
                ("Business Intelligence", "BI"),
                ("BI-Berater", "Business Intelligence"),
                ("Data Warehouse", "DWH"),
                ("Datawarehouse", "Data-Warehouse"),
                ("Data modelling", "Datenmodellierung"),
                ("Datenmodelle", "Data models"),
                ("Star schema", "Datenmodellierung"),
                ("ELT", "ETL"),
                ("ETL-Strecken", "ETL"),
                ("Data pipelines", "ETL"),
                ("Azure Data Factory", "ADF"),
                ("Azure Synapse Analytics", "Synapse"),
                ("Qlik Sense", "QlikView"),
                ("T-SQL", "SQL"),
                ("Kennzahlen", "KPIs"),
                ("Key Performance Indicators", "KPI"),
                ("Berichtswesen", "Reporting"),
                ("Data visualisation", "Datenvisualisierung"),
                ("Machine Learning", "Maschinelles Lernen"),
                ("Master Data Management", "MDM"),
                ("Datenbanken", "Databases"),
                ("Data Engineer", "Data Engineering"),
                ("Data analysis", "Datenanalyse"),
            ],
        );
    }

    #[test]
    fn false_friends_stay_apart() {
        assert_apart(
            &data(),
            &[
                ("DAX-Konzern", "DAX"),
                ("PowerPoint", "Power BI"),
                ("Warehouse Management", "Data Warehouse"),
                ("Sparkasse", "Spark"),
                ("Operational Excellence", "Excel"),
                ("Snowflake-Schema", "Snowflake"),
                ("Power Apps", "Power BI"),
            ],
        );
    }
}
