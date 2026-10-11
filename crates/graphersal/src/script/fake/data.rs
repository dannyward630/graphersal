//! The English word lists behind [`ScriptFake`](super::ScriptFake).
//!
//! Part of the reproducibility contract: the generated values of a seed depend on the order and
//! the length of every list, so an edit here changes what every seed produces (see the module
//! docs of `fake`). Names are common US given names and surnames; cities are real US cities with
//! their state. Phone numbers and e-mail domains are generated from reserved fictional ranges
//! (`555-01xx`, `example.com`), never from this data.

/// Given names.
#[rustfmt::skip]
pub(super) const FIRST_NAMES: &[&str] = &[
    "James", "Mary", "John", "Patricia", "Robert", "Jennifer", "Michael", "Linda", "William",
    "Elizabeth", "David", "Barbara", "Richard", "Susan", "Joseph", "Jessica", "Thomas", "Sarah",
    "Charles", "Karen", "Christopher", "Lisa", "Daniel", "Nancy", "Matthew", "Betty", "Anthony",
    "Margaret", "Mark", "Sandra", "Donald", "Ashley", "Steven", "Kimberly", "Paul", "Emily",
    "Andrew", "Donna", "Joshua", "Michelle", "Kenneth", "Carol", "Kevin", "Amanda", "Brian",
    "Dorothy", "George", "Melissa", "Timothy", "Deborah", "Ronald", "Stephanie", "Edward",
    "Rebecca", "Jason", "Sharon", "Jeffrey", "Laura", "Ryan", "Cynthia", "Jacob", "Kathleen",
    "Gary", "Amy", "Nicholas", "Angela", "Eric", "Shirley", "Jonathan", "Anna", "Stephen",
    "Brenda", "Larry", "Pamela", "Justin", "Emma", "Scott", "Nicole", "Brandon", "Helen",
    "Benjamin", "Samantha", "Samuel", "Katherine", "Gregory", "Christine", "Alexander", "Debra",
    "Frank", "Rachel", "Patrick", "Carolyn", "Raymond", "Janet", "Jack", "Catherine", "Dennis",
    "Maria", "Jerry", "Heather", "Tyler", "Diane", "Aaron", "Ruth", "Jose", "Julie", "Adam",
    "Olivia", "Nathan", "Joyce", "Henry", "Virginia", "Douglas", "Victoria", "Zachary", "Kelly",
    "Peter", "Lauren", "Kyle", "Christina", "Ethan", "Joan", "Walter", "Evelyn", "Noah", "Judith",
    "Jeremy", "Megan", "Christian", "Andrea", "Keith", "Cheryl", "Roger", "Hannah", "Terry",
    "Jacqueline", "Gerald", "Martha", "Harold", "Gloria", "Sean", "Teresa", "Austin", "Ann",
    "Carl", "Sara", "Arthur", "Madison", "Lawrence", "Frances", "Dylan", "Kathryn", "Jesse",
    "Janice", "Jordan", "Jean", "Bryan", "Abigail", "Billy", "Alice", "Joe", "Julia", "Bruce",
    "Judy", "Gabriel", "Sophia", "Logan", "Grace", "Albert", "Denise", "Willie", "Amber", "Alan",
    "Doris", "Juan", "Marilyn", "Wayne", "Danielle", "Elijah", "Beverly", "Randy", "Isabella",
    "Roy", "Theresa", "Vincent", "Diana", "Ralph", "Natalie", "Eugene", "Brittany", "Russell",
    "Charlotte", "Bobby", "Marie", "Mason", "Kayla", "Philip", "Alexis", "Louis", "Lori",
];

/// Surnames.
#[rustfmt::skip]
pub(super) const LAST_NAMES: &[&str] = &[
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis", "Rodriguez",
    "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson", "Thomas", "Taylor",
    "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson", "White", "Harris", "Sanchez",
    "Clark", "Ramirez", "Lewis", "Robinson", "Walker", "Young", "Allen", "King", "Wright",
    "Scott", "Torres", "Nguyen", "Hill", "Flores", "Green", "Adams", "Nelson", "Baker", "Hall",
    "Rivera", "Campbell", "Mitchell", "Carter", "Roberts", "Gomez", "Phillips", "Evans", "Turner",
    "Diaz", "Parker", "Cruz", "Edwards", "Collins", "Reyes", "Stewart", "Morris", "Morales",
    "Murphy", "Cook", "Rogers", "Gutierrez", "Ortiz", "Morgan", "Cooper", "Peterson", "Bailey",
    "Reed", "Kelly", "Howard", "Ramos", "Kim", "Cox", "Ward", "Richardson", "Watson", "Brooks",
    "Chavez", "Wood", "James", "Bennett", "Gray", "Mendoza", "Ruiz", "Hughes", "Price",
    "Alvarez", "Castillo", "Sanders", "Patel", "Myers", "Long", "Ross", "Foster", "Jimenez",
    "Powell", "Jenkins", "Perry", "Russell", "Sullivan", "Bell", "Coleman", "Butler", "Henderson",
    "Barnes", "Gonzales", "Fisher", "Vasquez", "Simmons", "Romero", "Jordan", "Patterson",
    "Alexander", "Hamilton", "Graham", "Reynolds", "Griffin", "Wallace", "Moreno", "West", "Cole",
    "Hayes", "Bryant", "Herrera", "Gibson", "Ellis", "Tran", "Medina", "Aguilar", "Stevens",
    "Murray", "Ford", "Castro", "Marshall", "Owens", "Harrison", "Fernandez", "McDonald",
    "Woods", "Washington", "Kennedy", "Wells", "Vargas", "Henry", "Chen", "Freeman", "Webb",
    "Tucker", "Guzman", "Burns", "Crawford", "Olson", "Simpson", "Porter", "Hunter", "Gordon",
    "Mendez", "Silva", "Shaw", "Snyder", "Mason", "Dixon", "Munoz", "Hunt", "Hicks", "Holmes",
    "Palmer", "Wagner", "Black", "Robertson", "Boyd", "Rose", "Stone", "Salazar", "Fox",
    "Warren", "Mills", "Meyer", "Rice", "Schmidt", "Garza", "Daniels", "Ferguson", "Nichols",
    "Stephens", "Soto", "Weaver", "Ryan", "Gardner", "Payne", "Grant", "Dunn", "Kelley",
];

/// US states: name and postal abbreviation.
#[rustfmt::skip]
pub(super) const STATES: &[(&str, &str)] = &[
    ("Alabama", "AL"), ("Alaska", "AK"), ("Arizona", "AZ"), ("Arkansas", "AR"),
    ("California", "CA"), ("Colorado", "CO"), ("Connecticut", "CT"), ("Delaware", "DE"),
    ("Florida", "FL"), ("Georgia", "GA"), ("Hawaii", "HI"), ("Idaho", "ID"), ("Illinois", "IL"),
    ("Indiana", "IN"), ("Iowa", "IA"), ("Kansas", "KS"), ("Kentucky", "KY"),
    ("Louisiana", "LA"), ("Maine", "ME"), ("Maryland", "MD"), ("Massachusetts", "MA"),
    ("Michigan", "MI"), ("Minnesota", "MN"), ("Mississippi", "MS"), ("Missouri", "MO"),
    ("Montana", "MT"), ("Nebraska", "NE"), ("Nevada", "NV"), ("New Hampshire", "NH"),
    ("New Jersey", "NJ"), ("New Mexico", "NM"), ("New York", "NY"), ("North Carolina", "NC"),
    ("North Dakota", "ND"), ("Ohio", "OH"), ("Oklahoma", "OK"), ("Oregon", "OR"),
    ("Pennsylvania", "PA"), ("Rhode Island", "RI"), ("South Carolina", "SC"),
    ("South Dakota", "SD"), ("Tennessee", "TN"), ("Texas", "TX"), ("Utah", "UT"),
    ("Vermont", "VT"), ("Virginia", "VA"), ("Washington", "WA"), ("West Virginia", "WV"),
    ("Wisconsin", "WI"), ("Wyoming", "WY"),
];

/// US cities with the postal abbreviation of their state (so an address stays consistent).
#[rustfmt::skip]
pub(super) const CITIES: &[(&str, &str)] = &[
    ("New York", "NY"), ("Los Angeles", "CA"), ("Chicago", "IL"), ("Houston", "TX"),
    ("Phoenix", "AZ"), ("Philadelphia", "PA"), ("San Antonio", "TX"), ("San Diego", "CA"),
    ("Dallas", "TX"), ("San Jose", "CA"), ("Austin", "TX"), ("Jacksonville", "FL"),
    ("Fort Worth", "TX"), ("Columbus", "OH"), ("Charlotte", "NC"), ("Indianapolis", "IN"),
    ("San Francisco", "CA"), ("Seattle", "WA"), ("Denver", "CO"), ("Oklahoma City", "OK"),
    ("Nashville", "TN"), ("Washington", "DC"), ("El Paso", "TX"), ("Las Vegas", "NV"),
    ("Boston", "MA"), ("Detroit", "MI"), ("Portland", "OR"), ("Louisville", "KY"),
    ("Memphis", "TN"), ("Baltimore", "MD"), ("Milwaukee", "WI"), ("Albuquerque", "NM"),
    ("Tucson", "AZ"), ("Fresno", "CA"), ("Sacramento", "CA"), ("Mesa", "AZ"),
    ("Atlanta", "GA"), ("Kansas City", "MO"), ("Colorado Springs", "CO"), ("Omaha", "NE"),
    ("Raleigh", "NC"), ("Miami", "FL"), ("Virginia Beach", "VA"), ("Long Beach", "CA"),
    ("Oakland", "CA"), ("Minneapolis", "MN"), ("Tulsa", "OK"), ("Bakersfield", "CA"),
    ("Tampa", "FL"), ("Arlington", "TX"), ("Wichita", "KS"), ("Aurora", "CO"),
    ("New Orleans", "LA"), ("Cleveland", "OH"), ("Honolulu", "HI"), ("Anaheim", "CA"),
    ("Henderson", "NV"), ("Orlando", "FL"), ("Lexington", "KY"), ("Stockton", "CA"),
    ("Riverside", "CA"), ("Irvine", "CA"), ("Cincinnati", "OH"), ("Santa Ana", "CA"),
    ("Newark", "NJ"), ("Saint Paul", "MN"), ("Pittsburgh", "PA"), ("Greensboro", "NC"),
    ("Lincoln", "NE"), ("Durham", "NC"), ("Plano", "TX"), ("Anchorage", "AK"),
    ("Jersey City", "NJ"), ("St. Louis", "MO"), ("Chandler", "AZ"), ("North Las Vegas", "NV"),
    ("Chula Vista", "CA"), ("Buffalo", "NY"), ("Gilbert", "AZ"), ("Madison", "WI"),
    ("Reno", "NV"), ("Toledo", "OH"), ("Fort Wayne", "IN"), ("Lubbock", "TX"),
    ("St. Petersburg", "FL"), ("Laredo", "TX"), ("Irving", "TX"), ("Chesapeake", "VA"),
    ("Glendale", "AZ"), ("Winston-Salem", "NC"), ("Port St. Lucie", "FL"), ("Scottsdale", "AZ"),
    ("Garland", "TX"), ("Boise", "ID"), ("Norfolk", "VA"), ("Spokane", "WA"),
    ("Richmond", "VA"), ("Fremont", "CA"), ("Huntsville", "AL"), ("Frisco", "TX"),
    ("Salt Lake City", "UT"), ("Des Moines", "IA"), ("Little Rock", "AR"), ("Providence", "RI"),
    ("Burlington", "VT"), ("Portland", "ME"), ("Charleston", "SC"), ("Savannah", "GA"),
];

/// Countries (English short names).
#[rustfmt::skip]
pub(super) const COUNTRIES: &[&str] = &[
    "United States", "Canada", "Mexico", "Brazil", "Argentina", "Chile", "Colombia", "Peru",
    "United Kingdom", "Ireland", "France", "Germany", "Spain", "Portugal", "Italy",
    "Netherlands", "Belgium", "Switzerland", "Austria", "Denmark", "Norway", "Sweden", "Finland",
    "Iceland", "Poland", "Czechia", "Hungary", "Greece", "Turkey", "Egypt", "Morocco", "Nigeria",
    "Kenya", "South Africa", "Ghana", "Ethiopia", "India", "Pakistan", "Bangladesh", "China",
    "Japan", "South Korea", "Vietnam", "Thailand", "Malaysia", "Singapore", "Indonesia",
    "Philippines", "Australia", "New Zealand", "Israel", "Saudi Arabia",
    "United Arab Emirates", "Jordan", "Estonia", "Latvia", "Lithuania", "Slovenia", "Croatia",
    "Romania",
];

/// The name part of a street.
#[rustfmt::skip]
pub(super) const STREET_ROOTS: &[&str] = &[
    "Main", "Oak", "Pine", "Maple", "Cedar", "Elm", "Washington", "Lake", "Hill", "Park",
    "Walnut", "Spring", "North", "Ridge", "Lincoln", "Church", "Willow", "Mill", "Sunset",
    "Jackson", "Cherry", "Highland", "Johnson", "Railroad", "Jefferson", "Center", "Franklin",
    "River", "Chestnut", "Meadow", "Forest", "Madison", "Adams", "Lakeview", "Valley", "Dogwood",
    "Hickory", "Birch", "Sycamore", "Magnolia", "Laurel", "Prospect", "Union", "Market",
    "Broad", "Front", "Water", "School", "College", "Green", "Bridge", "Harbor", "Mountain",
    "Orchard", "Aspen", "Spruce", "Poplar", "Cypress", "Hawthorn", "Beacon",
];

/// The kind of a street.
#[rustfmt::skip]
pub(super) const STREET_SUFFIXES: &[&str] = &[
    "Street", "Avenue", "Road", "Lane", "Drive", "Court", "Place", "Boulevard", "Way", "Circle",
    "Terrace", "Parkway", "Trail",
];

/// Seniority prefixes of a job title (empty: none).
#[rustfmt::skip]
pub(super) const JOB_LEVELS: &[&str] = &[
    "", "", "Junior", "Senior", "Lead", "Principal", "Associate", "Chief", "Staff",
];

/// Areas of a job title.
#[rustfmt::skip]
pub(super) const JOB_AREAS: &[&str] = &[
    "Software", "Data", "Product", "Marketing", "Sales", "Finance", "Operations", "Security",
    "Research", "Support", "Design", "Infrastructure", "Quality", "Legal", "People",
];

/// Roles of a job title.
#[rustfmt::skip]
pub(super) const JOB_ROLES: &[&str] = &[
    "Engineer", "Analyst", "Manager", "Designer", "Specialist", "Consultant", "Architect",
    "Director", "Coordinator", "Administrator", "Scientist", "Officer",
];

/// Legal-form and other suffixes of a company name.
#[rustfmt::skip]
pub(super) const COMPANY_SUFFIXES: &[&str] = &[
    "Inc", "LLC", "Group", "Ltd", "Corp", "Partners", "Holdings", "Labs", "Systems",
    "Industries", "Solutions", "Co",
];

/// Top-level domains of [`ScriptFake::domain_name`](super::ScriptFake).
#[rustfmt::skip]
pub(super) const TLDS: &[&str] = &["com", "net", "org", "io", "biz", "info"];

/// The reserved example domains (RFC 2606) e-mail addresses use.
#[rustfmt::skip]
pub(super) const EMAIL_DOMAINS: &[&str] = &["example.com", "example.org", "example.net"];

/// Plain English words for words, sentences and company names.
#[rustfmt::skip]
pub(super) const WORDS: &[&str] = &[
    "amber", "anchor", "apple", "arrow", "atlas", "autumn", "bamboo", "beacon", "birch", "blue",
    "bold", "bright", "bronze", "canyon", "cedar", "cloud", "coast", "comet", "copper", "coral",
    "crane", "crystal", "dawn", "delta", "desert", "eagle", "echo", "ember", "falcon", "field",
    "flint", "forest", "fox", "frost", "garden", "glacier", "golden", "granite", "green",
    "harbor", "hawk", "hazel", "horizon", "iron", "island", "ivory", "jade", "juniper", "lake",
    "lantern", "laurel", "lemon", "light", "lime", "linen", "lotus", "maple", "marble", "meadow",
    "mesa", "meteor", "mint", "moon", "moss", "mountain", "north", "oak", "ocean", "olive",
    "onyx", "orbit", "orchid", "otter", "pearl", "pepper", "pine", "pioneer", "planet", "polar",
    "prairie", "quartz", "quiet", "rain", "raven", "red", "ridge", "river", "rock", "rose",
    "ruby", "sage", "salt", "sand", "shadow", "silver", "sky", "slate", "snow", "solar",
    "spark", "spring", "spruce", "star", "stone", "storm", "summit", "sun", "swift", "thunder",
    "tide", "timber", "trail", "valley", "velvet", "violet", "wave", "west", "willow", "wind",
    "winter", "wolf", "yellow", "zephyr",
];

/// The classic Lorem ipsum vocabulary (pseudo-Latin) for `lorem_*` text.
#[rustfmt::skip]
pub(super) const LOREM_WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do",
    "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua", "enim",
    "ad", "minim", "veniam", "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi",
    "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute", "irure", "in",
    "reprehenderit", "voluptate", "velit", "esse", "cillum", "eu", "fugiat", "nulla",
    "pariatur", "excepteur", "sint", "occaecat", "cupidatat", "non", "proident", "sunt",
    "culpa", "qui", "officia", "deserunt", "mollit", "anim", "id", "est", "laborum", "integer",
    "vitae", "justo", "eget", "magnis", "dis", "parturient", "montes", "nascetur", "ridiculus",
    "mus", "mauris", "pharetra", "massa", "tincidunt", "nunc", "pulvinar", "sapien", "rhoncus",
    "urna", "neque", "viverra", "aliquet", "porttitor", "lacus", "luctus", "accumsan", "tortor",
    "posuere", "ac", "turpis", "egestas", "maecenas", "ultricies", "mi", "quam", "pellentesque",
    "nec", "nam", "libero", "volutpat", "blandit", "cursus", "risus", "feugiat", "pretium",
    "nibh", "ornare", "suspendisse", "sagittis", "vel", "facilisis", "gravida", "fermentum",
];
