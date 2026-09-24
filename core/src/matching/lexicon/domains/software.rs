//! Software and cloud: software development and languages, frameworks, cloud platforms,
//! DevOps and containers, architecture, agile roles, testing and APIs.
//!
//! external contract - do not translate.

use super::Domain;

pub(crate) const DOMAIN: Domain = Domain {
    name: "software",
    triggers: &[
        "angular",
        "ansible",
        "aws",
        "azure",
        "backend",
        "c#",
        "c++",
        "docker",
        "dotnet",
        "frontend",
        "full-stack",
        "fullstack",
        "github",
        "gitlab",
        "golang",
        "java",
        "jenkins",
        "k8s",
        "kotlin",
        "kubernetes",
        "microservice",
        "node.js",
        "python",
        "react",
        "softwarearchitekt",
        "softwareentwickl",
        "spring",
        "terraform",
        "typescript",
        "vue",
    ],
    generic: &[
        "develop",
        "development",
        "entwickel",
        "entwickl",
        "entwicklung",
        "programm",
    ],
    concepts: &[
        // Development and its roles.
        ("software development", "softwareentwicklung"),
        ("software engineering", "softwareentwicklung"),
        ("software engineer", "softwareentwicklung"),
        ("software developer", "softwareentwicklung"),
        ("softwareentwickler", "softwareentwicklung"),
        ("softwareentwicklerin", "softwareentwicklung"),
        ("application development", "softwareentwicklung"),
        ("anwendungsentwicklung", "softwareentwicklung"),
        ("programmierung", "softwareentwicklung"),
        ("programming", "softwareentwicklung"),
        ("back-end", "backend"),
        ("front-end", "frontend"),
        ("full stack", "fullstack"),
        ("full-stack", "fullstack"),
        ("tech lead", "tech-lead"),
        ("technical lead", "tech-lead"),
        ("lead developer", "tech-lead"),
        ("chief technology officer", "cto"),
        ("mobile development", "appentwicklung"),
        ("app development", "appentwicklung"),
        ("app entwicklung", "appentwicklung"),
        ("low code", "low-code"),
        // Languages and frameworks (`JavaScript` is not `Java`).
        ("java script", "javascript"),
        ("ecmascript", "javascript"),
        ("net", "dotnet"),
        ("net core", "dotnet"),
        ("asp.net", "dotnet"),
        ("cpp", "c++"),
        ("python3", "python"),
        ("react.js", "react"),
        ("reactjs", "react"),
        ("angularjs", "angular"),
        ("vue.js", "vue"),
        ("vuejs", "vue"),
        ("nodejs", "node.js"),
        ("spring boot", "spring-boot"),
        ("springboot", "spring-boot"),
        // Cloud platforms.
        ("cloud computing", "cloud"),
        ("amazon web services", "aws"),
        ("microsoft azure", "azure"),
        ("google cloud platform", "google-cloud"),
        ("google cloud", "google-cloud"),
        ("gcp", "google-cloud"),
        ("cloud architecture", "cloud-architektur"),
        ("cloud architect", "cloud-architektur"),
        ("cloudarchitektur", "cloud-architektur"),
        ("cloud migration", "cloud-migration"),
        ("cloudmigration", "cloud-migration"),
        ("multi cloud", "multi-cloud"),
        // DevOps and platforms.
        ("dev ops", "devops"),
        ("continuous integration", "ci-cd"),
        ("continuous delivery", "ci-cd"),
        ("continuous deployment", "ci-cd"),
        ("cicd", "ci-cd"),
        ("k8s", "kubernetes"),
        ("infrastructure code", "infrastructure-as-code"),
        ("iac", "infrastructure-as-code"),
        ("micro services", "microservices"),
        // Architecture.
        ("software architecture", "softwarearchitektur"),
        ("software architect", "softwarearchitektur"),
        ("softwarearchitekt", "softwarearchitektur"),
        ("softwarearchitektin", "softwarearchitektur"),
        ("solution architecture", "solutionarchitektur"),
        ("solution architect", "solutionarchitektur"),
        ("losungsarchitektur", "solutionarchitektur"),
        ("enterprise architecture", "enterprisearchitektur"),
        ("enterprise architect", "enterprisearchitektur"),
        ("unternehmensarchitektur", "enterprisearchitektur"),
        ("it-architecture", "it-architektur"),
        ("it-architect", "it-architektur"),
        // Agile roles and methods.
        ("scrum master", "scrum-master"),
        ("scrummaster", "scrum-master"),
        ("product owner", "product-owner"),
        ("po", "product-owner"),
        ("product management", "produktmanagement"),
        ("produktmanager", "produktmanagement"),
        ("produktmanagerin", "produktmanagement"),
        ("scaled agile framework", "safe"),
        ("scaled agile", "safe"),
        ("agile coach", "agile-coach"),
        // Testing and interfaces.
        ("software testing", "softwaretest"),
        ("test automation", "testautomatisierung"),
        ("unit testing", "unit-test"),
        ("test driven development", "tdd"),
        ("apis", "api"),
        ("rest api", "rest-api"),
        ("rest apis", "rest-api"),
        ("restful", "rest-api"),
    ],
};

#[cfg(test)]
mod tests {
    use super::super::testing::{assert_apart, assert_same, vocab};
    use crate::matching::atoms::Vocab;

    fn software() -> Vocab {
        let v = vocab(&["Java", "Spring Boot", "Kubernetes", "AWS"]);
        assert_eq!(v.packs(), ["software"]);
        v
    }

    #[test]
    fn paraphrases() {
        assert_same(
            &software(),
            &[
                ("Software Development", "Softwareentwicklung"),
                ("Software Engineer", "Softwareentwicklerin"),
                ("Java Script", "JavaScript"),
                (".NET", "dotnet"),
                (".NET Core", "ASP.NET"),
                ("React.js", "React"),
                ("Node.js", "NodeJS"),
                ("Spring Boot", "Spring-Boot"),
                ("Amazon Web Services", "AWS"),
                ("Microsoft Azure", "Azure"),
                ("Google Cloud Platform", "GCP"),
                ("Cloud-Architektur", "Cloud Architecture"),
                ("Continuous Integration", "CICD"),
                ("K8s", "Kubernetes"),
                ("Microservices", "Micro Services"),
                ("Software Architect", "Softwarearchitektur"),
                ("Solution Architect", "Solution Architecture"),
                ("Enterprise Architecture", "Unternehmensarchitektur"),
                ("Scrum Master", "ScrumMaster"),
                ("Product Owner", "PO"),
                ("Produktmanagerin", "Product Management"),
                ("Scaled Agile Framework", "SAFe"),
                ("Test automation", "Testautomatisierung"),
                ("REST APIs", "RESTful"),
            ],
        );
    }

    #[test]
    fn false_friends_stay_apart() {
        assert_apart(
            &software(),
            &[
                ("JavaScript", "Java"),
                ("Java", "JavaScript"),
                ("Java Script", "Java"),
                ("Nettolohn", ".NET"),
                ("Softwareentwicklung", "Software"),
                ("Organisationsentwicklung", "Softwareentwicklung"),
                ("Solution Design", "Solution Architect"),
                ("Restrukturierung", "REST API"),
            ],
        );
    }
}
