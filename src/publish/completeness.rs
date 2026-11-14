//! How much of a reference `_Data.xml` a generated one states.
//!
//! [`super::diff_pid_export`] asks whether anything a run wrote is wrong and
//! whether anything it left out is unexcused. A run can pass that while
//! stating half of what the reference does: what a `.pid` or a dump cannot
//! state is excused class by class, so a class added to the excuses and a
//! class written out read the same there.
//!
//! [`measure`] counts against the whole reference. Every object, interface
//! element, attribute and relation the reference carries lands in one of the
//! denominators, whether or not anything excuses it: this module reads the
//! two documents and nothing else — not the catalog that lists the excuses —
//! so a share moves only when the generated document states more of the
//! reference or less of it.
//!
//! What counts as stated:
//!
//! * an **object** — a top-level block other than `<Rel>`, keyed by its
//!   `IObject UID` — when the generated document has a block of the same
//!   class under the same UID. A `PIDPipeline` whose UID the generated
//!   document lacks pairs with a generated `PIDPipeline` of the same
//!   `IObject Name`: a dump with no pipeline rows leaves a run no UID to
//!   restate, and [`Completeness::objects_paired_by_name`] says how many
//!   objects are stated that way;
//! * an **interface element** of an object (`<IPBSItem …/>`, and an empty
//!   `<IDrawingItem/>` just the same) when the paired block has an element
//!   of that name;
//! * a **non-empty value** when the paired block states the same attribute
//!   of the same interface with the same value. `IObject UID` is the
//!   object's key and not one of its values;
//! * an **empty attribute** (`InstrTagSuffix=""`) when the paired block
//!   states that attribute, empty as well;
//! * a **relation** when the generated document has a `<Rel>` with the same
//!   `UID1`, `UID2` and `DefUID`, an end that is a name-paired pipeline
//!   reading as its pair's UID. One generated relation answers for one
//!   reference relation.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use quick_xml::errors::IllFormedError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::diff::{MalformedPublishDocument, PublishDocumentSide};

/// The one class whose objects pair by name when the UIDs differ.
const PIPELINE: &str = "PIDPipeline";

/// How many of the items a reference carries the generated document states.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Share {
    /// Reference items the generated document states too.
    pub stated: usize,
    /// Items the reference carries.
    pub reference: usize,
}

impl Share {
    /// Reference items the generated document does not state.
    #[must_use]
    pub fn missing(self) -> usize {
        self.reference.saturating_sub(self.stated)
    }

    fn count(&mut self, stated: bool) {
        self.reference += 1;
        self.stated += usize::from(stated);
    }
}

impl fmt::Display for Share {
    /// `stated / reference (percent)`, the percentage to one decimal. A share
    /// short of the whole never rounds up to `100.0%`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { stated, reference } = *self;
        if reference == 0 {
            return write!(f, "{stated} / {reference}");
        }
        let (wide_stated, wide_reference) = (stated as u128, reference as u128);
        let mut tenths = (wide_stated * 1000 + wide_reference / 2) / wide_reference;
        if stated < reference {
            tenths = tenths.min(999);
        }
        write!(
            f,
            "{stated} / {reference} ({}.{}%)",
            tenths / 10,
            tenths % 10
        )
    }
}

/// A `<Container>` attribute of the reference the generated document states
/// with another value or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDifference {
    /// The attribute's name.
    pub attribute: String,
    /// What the generated document states; `None` when it lacks the attribute.
    pub generated: Option<String>,
    /// What the reference states.
    pub reference: String,
}

/// What a generated `_Data.xml` states of a reference one: a [`Share`] for
/// each kind of content, and the same shares broken down by class.
///
/// Every denominator is the reference's own count. See the module
/// documentation for what counts as stated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completeness {
    /// `<Container>` attributes the generated document states with the
    /// reference's value.
    pub container: Share,
    /// The `<Container>` attributes that are not, in document order.
    pub container_differences: Vec<ContainerDifference>,
    /// Objects: top-level blocks other than `<Rel>`.
    pub objects: Share,
    /// How many of the stated objects are `PIDPipeline` blocks paired by
    /// `IObject Name` under a UID of the generated document's own.
    pub objects_paired_by_name: usize,
    /// Interface elements, one per interface name an object carries.
    pub interfaces: Share,
    /// Attribute values that are not empty, `IObject UID` left out.
    pub values: Share,
    /// Of the non-empty values that are not stated, how many the generated
    /// document states with another non-empty value.
    pub values_different: usize,
    /// Attributes the reference states empty.
    pub empty_attributes: Share,
    /// Relations, by their two ends and `DefUID`.
    pub relations: Share,
    /// Reference relations the generated document states under the
    /// reference's own `<Rel>` UID and with the reference's own end UIDs.
    pub relations_verbatim: usize,
    /// Interface elements on stated objects that the reference's block does
    /// not have. Not a share of the reference: content the generated
    /// document adds to it.
    pub interfaces_not_in_reference: usize,
    /// [`Self::objects`] by class.
    pub objects_by_class: BTreeMap<String, Share>,
    /// [`Self::interfaces`] by `(class, interface)`.
    pub interfaces_by_class: BTreeMap<(String, String), Share>,
    /// [`Self::values`] by `(class, interface, attribute)`.
    pub values_by_attribute: BTreeMap<(String, String, String), Share>,
    /// [`Self::values_different`] by `(class, interface, attribute)`.
    pub values_different_by_attribute: BTreeMap<(String, String, String), usize>,
    /// [`Self::empty_attributes`] by `(class, interface, attribute)`.
    pub empty_attributes_by_attribute: BTreeMap<(String, String, String), Share>,
    /// [`Self::relations`] by `DefUID`.
    pub relations_by_def_uid: BTreeMap<String, Share>,
    /// [`Self::interfaces_not_in_reference`] by `(class, interface)`.
    pub interfaces_not_in_reference_by_class: BTreeMap<(String, String), usize>,
}

impl Completeness {
    /// How many attributes of the reference document the shares account for:
    /// one per `<Container>` attribute, one `UID` per object, one per
    /// non-empty value, one per empty attribute, and the four a `<Rel>`
    /// carries (`UID`, `UID1`, `UID2`, `DefUID`).
    ///
    /// When this equals the number of attributes in the reference file, the
    /// denominators are the whole reference. A block with no `IObject UID`,
    /// an element nested deeper than an interface, or a `<Rel>` with a fifth
    /// attribute would leave the file with more.
    #[must_use]
    pub fn reference_attributes(&self) -> usize {
        self.container.reference
            + self.objects.reference
            + self.values.reference
            + self.empty_attributes.reference
            + 4 * self.relations.reference
    }
}

impl fmt::Display for Completeness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "container           {}", self.container)?;
        writeln!(
            f,
            "objects             {}, {} paired by name",
            self.objects, self.objects_paired_by_name
        )?;
        writeln!(f, "interface elements  {}", self.interfaces)?;
        writeln!(
            f,
            "non-empty values    {}, {} stated differently",
            self.values, self.values_different
        )?;
        writeln!(f, "empty attributes    {}", self.empty_attributes)?;
        writeln!(
            f,
            "relations           {}, {} verbatim",
            self.relations, self.relations_verbatim
        )?;
        write!(
            f,
            "not in the reference: {} interface elements",
            self.interfaces_not_in_reference
        )
    }
}

/// Measure how much of `reference_xml` the document `generated_xml` states.
///
/// # Errors
///
/// Returns [`MalformedPublishDocument`] when either document is not
/// well-formed XML; nothing is measured then.
pub fn measure(
    generated_xml: &str,
    reference_xml: &str,
) -> Result<Completeness, MalformedPublishDocument> {
    let generated = read_document(generated_xml, PublishDocumentSide::Generated)?;
    let reference = read_document(reference_xml, PublishDocumentSide::Reference)?;
    let mut measured = Completeness::default();
    measure_container(&generated, &reference, &mut measured);
    let pairing = pair_objects(&generated, &reference);
    measured.objects_paired_by_name = pairing.renamed.len();
    for (object, partner) in reference.objects.iter().zip(&pairing.partners) {
        measure_object(object, *partner, &mut measured);
    }
    measure_relations(&generated, &reference, &pairing, &mut measured);
    Ok(measured)
}

fn measure_container(generated: &Document, reference: &Document, measured: &mut Completeness) {
    for (attribute, value) in &reference.container {
        let ours = generated
            .container
            .iter()
            .find(|(name, _)| name == attribute)
            .map(|(_, value)| value);
        let stated = ours == Some(value);
        measured.container.count(stated);
        if !stated {
            measured.container_differences.push(ContainerDifference {
                attribute: attribute.clone(),
                generated: ours.cloned(),
                reference: value.clone(),
            });
        }
    }
}

/// Which generated block answers for each reference object.
struct Pairing<'a> {
    /// The generated block paired with each reference object, in the
    /// reference's document order.
    partners: Vec<Option<&'a Object>>,
    /// Reference pipeline UID to the UID of the generated pipeline that
    /// pairs with it by name.
    renamed: HashMap<&'a str, &'a str>,
}

fn pair_objects<'a>(generated: &'a Document, reference: &'a Document) -> Pairing<'a> {
    let by_uid: HashMap<&str, &Object> = generated
        .objects
        .iter()
        .map(|object| (object.uid.as_str(), object))
        .collect();
    let reference_uids: HashSet<&str> = reference
        .objects
        .iter()
        .map(|object| object.uid.as_str())
        .collect();
    // A generated pipeline is free to pair by name when the reference has no
    // object under its UID; of two with one name, the first in the document.
    let mut by_name: HashMap<&str, &Object> = HashMap::new();
    for object in &generated.objects {
        if object.class != PIPELINE || reference_uids.contains(object.uid.as_str()) {
            continue;
        }
        if let Some(name) = object.name() {
            by_name.entry(name).or_insert(object);
        }
    }
    let mut renamed = HashMap::new();
    let partners = reference
        .objects
        .iter()
        .map(|object| match by_uid.get(object.uid.as_str()) {
            Some(found) => (found.class == object.class).then_some(*found),
            None if object.class == PIPELINE => {
                let found = object.name().and_then(|name| by_name.remove(name))?;
                renamed.insert(object.uid.as_str(), found.uid.as_str());
                Some(found)
            }
            None => None,
        })
        .collect();
    Pairing { partners, renamed }
}

fn measure_object(object: &Object, partner: Option<&Object>, measured: &mut Completeness) {
    let class = &object.class;
    measured.objects.count(partner.is_some());
    measured
        .objects_by_class
        .entry(class.clone())
        .or_default()
        .count(partner.is_some());
    for (interface, attributes) in &object.interfaces {
        let ours = partner.and_then(|partner| partner.interfaces.get(interface));
        measured.interfaces.count(ours.is_some());
        measured
            .interfaces_by_class
            .entry((class.clone(), interface.clone()))
            .or_default()
            .count(ours.is_some());
        for (attribute, value) in attributes {
            let ours = ours.and_then(|ours| ours.get(attribute));
            let key = (class.clone(), interface.clone(), attribute.clone());
            if value.is_empty() {
                let stated = ours.is_some_and(String::is_empty);
                measured.empty_attributes.count(stated);
                measured
                    .empty_attributes_by_attribute
                    .entry(key)
                    .or_default()
                    .count(stated);
                continue;
            }
            let stated = ours == Some(value);
            measured.values.count(stated);
            measured
                .values_by_attribute
                .entry(key.clone())
                .or_default()
                .count(stated);
            if !stated && ours.is_some_and(|ours| !ours.is_empty()) {
                measured.values_different += 1;
                *measured
                    .values_different_by_attribute
                    .entry(key)
                    .or_default() += 1;
            }
        }
    }
    let Some(partner) = partner else {
        return;
    };
    for interface in partner.interfaces.keys() {
        if !object.interfaces.contains_key(interface) {
            measured.interfaces_not_in_reference += 1;
            *measured
                .interfaces_not_in_reference_by_class
                .entry((class.clone(), interface.clone()))
                .or_default() += 1;
        }
    }
}

fn measure_relations(
    generated: &Document,
    reference: &Document,
    pairing: &Pairing<'_>,
    measured: &mut Completeness,
) {
    let mut unclaimed: HashMap<(&str, &str, &str), usize> = HashMap::new();
    let mut verbatim: HashSet<(&str, &str, &str, &str)> = HashSet::new();
    for relation in &generated.relations {
        *unclaimed.entry(relation.wiring()).or_default() += 1;
        verbatim.insert((
            relation.uid.as_str(),
            relation.uid1.as_str(),
            relation.uid2.as_str(),
            relation.def_uid.as_str(),
        ));
    }
    for relation in &reference.relations {
        let (uid1, uid2, def_uid) = relation.wiring();
        let end = |uid| pairing.renamed.get(uid).copied().unwrap_or(uid);
        let stated = match unclaimed.get_mut(&(end(uid1), end(uid2), def_uid)) {
            Some(left) if *left > 0 => {
                *left -= 1;
                true
            }
            _ => false,
        };
        measured.relations.count(stated);
        measured
            .relations_by_def_uid
            .entry(relation.def_uid.clone())
            .or_default()
            .count(stated);
        if verbatim.contains(&(relation.uid.as_str(), uid1, uid2, def_uid)) {
            measured.relations_verbatim += 1;
        }
    }
}

/// One `_Data.xml`, read for what it states and nothing about how.
#[derive(Default)]
struct Document {
    /// The `<Container>` tag's attributes, in document order.
    container: Vec<(String, String)>,
    /// Top-level blocks other than `<Rel>`, in document order.
    objects: Vec<Object>,
    relations: Vec<Relation>,
}

struct Object {
    uid: String,
    class: String,
    /// Interface name to its attributes. `IObject UID` is [`Self::uid`] and
    /// is not repeated here; an element written twice is one interface.
    interfaces: BTreeMap<String, BTreeMap<String, String>>,
}

impl Object {
    /// The object's `IObject Name`, when it states a non-empty one.
    fn name(&self) -> Option<&str> {
        self.interfaces
            .get("IObject")
            .and_then(|attributes| attributes.get("Name"))
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }
}

struct Relation {
    uid: String,
    uid1: String,
    uid2: String,
    def_uid: String,
}

impl Relation {
    fn wiring(&self) -> (&str, &str, &str) {
        (&self.uid1, &self.uid2, &self.def_uid)
    }
}

type Attributes = Vec<(String, String)>;
/// A top-level block being read: its tag and its interface elements.
type OpenBlock = (String, Vec<(String, Attributes)>);

fn read_document(
    xml: &str,
    side: PublishDocumentSide,
) -> Result<Document, MalformedPublishDocument> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut document = Document::default();
    let mut open_tags: Vec<String> = Vec::new();
    let mut block: Option<OpenBlock> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let tag = start.name().as_ref().to_string();
                let attributes = attributes_of(&start, side, reader.buffer_position())?;
                place(&mut document, &mut block, open_tags.len(), &tag, attributes);
                open_tags.push(tag);
            }
            Ok(Event::Empty(start)) => {
                let tag = start.name().as_ref().to_string();
                let attributes = attributes_of(&start, side, reader.buffer_position())?;
                place(&mut document, &mut block, open_tags.len(), &tag, attributes);
                if open_tags.len() == 1 {
                    close_block(&mut document, block.take());
                }
            }
            Ok(Event::End(_)) => {
                open_tags.pop();
                if open_tags.len() == 1 {
                    close_block(&mut document, block.take());
                }
            }
            Ok(Event::Eof) => {
                // quick-xml reaches the end of input without checking that
                // every start tag was closed; a document cut between tags
                // ends here.
                if let Some(tag) = open_tags.pop() {
                    let error = quick_xml::Error::IllFormed(IllFormedError::MissingEndTag(tag));
                    return Err(MalformedPublishDocument {
                        side,
                        position: reader.buffer_position(),
                        message: error.to_string(),
                    });
                }
                return Ok(document);
            }
            Err(error) => {
                return Err(MalformedPublishDocument {
                    side,
                    position: reader.error_position(),
                    message: error.to_string(),
                });
            }
            _ => {}
        }
    }
}

/// File an element that opens with `depth` elements already open: the root
/// gives the container's attributes, its children are the blocks, and their
/// children the interface elements.
fn place(
    document: &mut Document,
    block: &mut Option<OpenBlock>,
    depth: usize,
    tag: &str,
    attributes: Attributes,
) {
    match depth {
        0 => document.container = attributes,
        1 => *block = Some((tag.to_string(), Vec::new())),
        2 => {
            if let Some((_, interfaces)) = block.as_mut() {
                interfaces.push((tag.to_string(), attributes));
            }
        }
        _ => {}
    }
}

/// The attributes of one opening tag as (name, unescaped value) pairs in
/// document order. A malformed attribute or an unknown entity refuses the
/// document at `at` rather than dropping the value.
fn attributes_of(
    start: &BytesStart<'_>,
    side: PublishDocumentSide,
    at: u64,
) -> Result<Attributes, MalformedPublishDocument> {
    let refuse = |error: quick_xml::Error| MalformedPublishDocument {
        side,
        position: at,
        message: error.to_string(),
    };
    start
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(|error| refuse(error.into()))?;
            let name = attribute.key.as_ref().to_string();
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(refuse)?
                .into_owned();
            Ok((name, value))
        })
        .collect()
}

/// Turn a finished block into a relation or an object. A block with no
/// `IObject UID` has no key to compare under and is left out;
/// [`Completeness::reference_attributes`] is how a caller notices one.
fn close_block(document: &mut Document, block: Option<OpenBlock>) {
    let Some((class, elements)) = block else {
        return;
    };
    let mut uid = None;
    let mut interfaces: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (interface, attributes) in elements {
        let stated = interfaces.entry(interface.clone()).or_default();
        for (attribute, value) in attributes {
            if interface == "IObject" && attribute == "UID" {
                uid = Some(value);
            } else {
                stated.insert(attribute, value);
            }
        }
    }
    let Some(uid) = uid else {
        return;
    };
    if class == "Rel" {
        let mut end = |attribute: &str| {
            interfaces
                .get_mut("IRel")
                .and_then(|attributes| attributes.remove(attribute))
                .unwrap_or_default()
        };
        let (uid1, uid2, def_uid) = (end("UID1"), end("UID2"), end("DefUID"));
        document.relations.push(Relation {
            uid,
            uid1,
            uid2,
            def_uid,
        });
    } else {
        document.objects.push(Object {
            uid,
            class,
            interfaces,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::catalog::EXCLUDED_ATTRIBUTE_CLASSES;
    use crate::publish::diff_pid_export;

    /// A `_Data.xml` with the given container attributes and blocks.
    fn document(container: &str, blocks: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
             <Container {container}>\n{blocks}</Container>\n"
        )
    }

    /// The attributes a file carries, counted off its text: every `="` an
    /// element opens a value with, less the XML declaration's own.
    fn attributes_in(xml: &str) -> usize {
        let declaration = xml.find("?>").map_or(0, |end| end + 2);
        xml[declaration..].matches("=\"").count()
    }

    const CONTAINER: &str = r#"CompSchema="PIDComponent" Plant="P01" DocUID="D1" DocName="Sheet""#;

    /// A drawing, a vessel with a nozzle, a named pipeline and the relations
    /// between them.
    const REFERENCE_BLOCKS: &str = r#"  <PIDDrawing>
    <IObject UID="D1" Name="Sheet"/>
    <IDocument DocCategory="P&amp;ID Documents" DocTitle=""/>
  </PIDDrawing>
  <PIDProcessVessel>
    <IObject UID="V1" Name="V-101" Description="Separator"/>
    <IPBSItem ConstructionStatus="@New" ConstructionStatus2=""/>
    <IDrawingItem/>
  </PIDProcessVessel>
  <PIDNozzle>
    <IObject UID="N1" Name="N1"/>
    <IDrawingItem/>
  </PIDNozzle>
  <PIDPipeline>
    <IObject UID="REFERENCE-PIPELINE" Name="P-1001"/>
    <IFluidSystem FluidCode="P"/>
  </PIDPipeline>
  <Rel>
    <IObject UID="R1"/>
    <IRel UID1="V1" UID2="N1" DefUID="EquipmentComponentComposition"/>
  </Rel>
  <Rel>
    <IObject UID="R2"/>
    <IRel UID1="N1" UID2="REFERENCE-PIPELINE" DefUID="PipingConnectors"/>
  </Rel>
"#;

    fn reference() -> String {
        document(CONTAINER, REFERENCE_BLOCKS)
    }

    #[test]
    fn a_document_states_all_of_itself() {
        let measured = measure(&reference(), &reference()).expect("well-formed");
        let whole = |share: Share, count: usize| {
            assert_eq!(
                share,
                Share {
                    stated: count,
                    reference: count
                }
            );
        };
        whole(measured.container, 4);
        whole(measured.objects, 4);
        // IObject on each of the four, IDocument, IPBSItem, IDrawingItem
        // twice, IFluidSystem.
        whole(measured.interfaces, 9);
        // Four names, the description, the category, the status, the fluid.
        whole(measured.values, 8);
        whole(measured.empty_attributes, 2);
        whole(measured.relations, 2);
        assert_eq!(measured.relations_verbatim, 2);
        assert_eq!(measured.objects_paired_by_name, 0);
        assert_eq!(measured.values_different, 0);
        assert_eq!(measured.interfaces_not_in_reference, 0);
        assert!(measured.container_differences.is_empty());
    }

    #[test]
    fn every_attribute_of_the_reference_is_in_a_denominator() {
        let reference = reference();
        // Measured against a document that states nothing, so no share's
        // numerator can stand in for its denominator.
        let nothing = document("", "");
        let measured = measure(&nothing, &reference).expect("well-formed");
        assert_eq!(measured.reference_attributes(), attributes_in(&reference));
        assert_eq!(attributes_in(&reference), 4 + 4 + 8 + 2 + 4 * 2);
        for share in [
            measured.container,
            measured.objects,
            measured.interfaces,
            measured.values,
            measured.empty_attributes,
            measured.relations,
        ] {
            assert_eq!(share.stated, 0);
            assert_eq!(share.missing(), share.reference);
        }
    }

    #[test]
    fn what_the_generated_document_leaves_out_is_counted_missing() {
        // No nozzle, no relation to it, no IPBSItem on the vessel, another
        // description, and the plant under another name.
        let generated = document(
            r#"CompSchema="PIDComponent" Plant="P02" DocUID="D1""#,
            r#"  <PIDDrawing>
    <IObject UID="D1" Name="Sheet"/>
    <IDocument DocCategory="P&amp;ID Documents"/>
  </PIDDrawing>
  <PIDProcessVessel>
    <IObject UID="V1" Name="V-101" Description="Drum"/>
    <IDrawingItem/>
  </PIDProcessVessel>
"#,
        );
        let measured = measure(&generated, &reference()).expect("well-formed");
        let share = |stated, reference| Share { stated, reference };
        assert_eq!(measured.container, share(2, 4));
        assert_eq!(
            measured.container_differences,
            vec![
                ContainerDifference {
                    attribute: "Plant".to_string(),
                    generated: Some("P02".to_string()),
                    reference: "P01".to_string(),
                },
                ContainerDifference {
                    attribute: "DocName".to_string(),
                    generated: None,
                    reference: "Sheet".to_string(),
                },
            ]
        );
        assert_eq!(measured.objects, share(2, 4));
        assert_eq!(measured.objects_by_class["PIDNozzle"], share(0, 1));
        // The drawing's two, the vessel's IObject and IDrawingItem.
        assert_eq!(measured.interfaces, share(4, 9));
        let vessel = |interface: &str| ("PIDProcessVessel".to_string(), interface.to_string());
        assert_eq!(
            measured.interfaces_by_class[&vessel("IPBSItem")],
            share(0, 1)
        );
        // The drawing's name and category, the vessel's name.
        assert_eq!(measured.values, share(3, 8));
        assert_eq!(measured.values_different, 1);
        let description = (
            "PIDProcessVessel".to_string(),
            "IObject".to_string(),
            "Description".to_string(),
        );
        assert_eq!(measured.values_by_attribute[&description], share(0, 1));
        assert_eq!(measured.values_different_by_attribute[&description], 1);
        // `DocTitle=""` is left out, and so is the empty status.
        assert_eq!(measured.empty_attributes, share(0, 2));
        assert_eq!(measured.relations, share(0, 2));
        assert_eq!(measured.relations_verbatim, 0);
    }

    #[test]
    fn an_empty_attribute_is_stated_only_by_an_empty_one() {
        let block = |title: &str| {
            document(
                "",
                &format!(
                    "  <PIDDrawing>\n    <IObject UID=\"D1\"/>\n    <IDocument{title}/>\n  \
                     </PIDDrawing>\n"
                ),
            )
        };
        let reference = block(r#" DocTitle="""#);
        let stated = |generated: &str| {
            measure(generated, &reference)
                .expect("well-formed")
                .empty_attributes
                .stated
        };
        assert_eq!(stated(&block(r#" DocTitle="""#)), 1);
        assert_eq!(stated(&block("")), 0);
        // A value where the reference has none is not the reference's empty
        // attribute, and no value of the reference's was stated differently.
        let filled = measure(&block(r#" DocTitle="Title""#), &reference).expect("well-formed");
        assert_eq!(filled.empty_attributes.stated, 0);
        assert_eq!(filled.values_different, 0);
    }

    #[test]
    fn a_pipeline_pairs_by_name_and_its_relations_follow() {
        let generated = reference()
            .replace("REFERENCE-PIPELINE", "OUR-PIPELINE")
            .replace("UID=\"R2\"", "UID=\"OUR-R2\"");
        let measured = measure(&generated, &reference()).expect("well-formed");
        assert_eq!(measured.objects.stated, 4);
        assert_eq!(measured.objects_paired_by_name, 1);
        assert_eq!(measured.values.stated, 8);
        assert_eq!(measured.relations.stated, 2);
        // The relation to the pipeline is stated, under ends of our own.
        assert_eq!(measured.relations_verbatim, 1);

        // Under another name the pipeline is another pipeline.
        let renamed = generated.replace("P-1001", "P-1002");
        let measured = measure(&renamed, &reference()).expect("well-formed");
        assert_eq!(measured.objects.stated, 3);
        assert_eq!(measured.objects_paired_by_name, 0);
        assert_eq!(measured.relations.stated, 1);
    }

    #[test]
    fn only_a_pipeline_pairs_by_name() {
        let generated = reference().replace("UID=\"N1\" Name=\"N1\"", "UID=\"N2\" Name=\"N1\"");
        let measured = measure(&generated, &reference()).expect("well-formed");
        assert_eq!(measured.objects.stated, 3);
        assert_eq!(measured.objects_by_class["PIDNozzle"].stated, 0);
    }

    #[test]
    fn a_block_of_another_class_does_not_state_the_object() {
        let generated = reference().replace("PIDNozzle>", "PIDPipingPort>");
        let measured = measure(&generated, &reference()).expect("well-formed");
        assert_eq!(measured.objects.stated, 3);
        assert_eq!(
            measured.objects_by_class["PIDNozzle"],
            Share {
                stated: 0,
                reference: 1
            }
        );
        // Its interface elements go with it.
        assert_eq!(measured.interfaces.stated, 7);
    }

    #[test]
    fn one_generated_relation_answers_for_one_reference_relation() {
        let relation = |uid: &str| {
            format!(
                "  <Rel>\n    <IObject UID=\"{uid}\"/>\n    <IRel UID1=\"A\" UID2=\"B\" \
                 DefUID=\"DrawingItems\"/>\n  </Rel>\n"
            )
        };
        let twice = document("", &format!("{}{}", relation("R1"), relation("R2")));
        let once = document("", &relation("OURS"));
        let measured = measure(&once, &twice).expect("well-formed");
        assert_eq!(
            measured.relations,
            Share {
                stated: 1,
                reference: 2
            }
        );
        assert_eq!(measured.relations_verbatim, 0);
        // The ends in the other order are another relation.
        let swapped = once.replace("UID1=\"A\" UID2=\"B\"", "UID1=\"B\" UID2=\"A\"");
        let measured = measure(&swapped, &twice).expect("well-formed");
        assert_eq!(measured.relations.stated, 0);
    }

    #[test]
    fn an_interface_the_reference_block_lacks_is_counted_apart() {
        let generated = reference().replace(
            "<IObject UID=\"N1\" Name=\"N1\"/>",
            "<IObject UID=\"N1\" Name=\"N1\"/>\n    <IOffpageConnector/>",
        );
        let measured = measure(&generated, &reference()).expect("well-formed");
        assert_eq!(measured.interfaces_not_in_reference, 1);
        let key = ("PIDNozzle".to_string(), "IOffpageConnector".to_string());
        assert_eq!(measured.interfaces_not_in_reference_by_class[&key], 1);
        // Nothing of the reference's is stated any less for it.
        assert_eq!(measured.interfaces.stated, measured.interfaces.reference);
    }

    /// What the gate excuses, the measure still counts: a reference object
    /// carrying every attribute of the exclusion list, published with none of
    /// them, passes the gate with nothing missing and states none of them.
    #[test]
    fn an_excused_attribute_is_still_in_the_denominator() {
        let mut elements: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for class in EXCLUDED_ATTRIBUTE_CLASSES {
            for (interface, attributes) in class.members {
                elements
                    .entry(*interface)
                    .or_default()
                    .extend(attributes.iter().copied());
            }
        }
        let excused: usize = elements.values().map(Vec::len).sum();
        assert!(excused > 0, "the exclusion list is not empty");
        let mut reference_block = String::from("  <PIDPipingComponent>\n");
        for (interface, attributes) in &elements {
            let uid = if *interface == "IObject" {
                " UID=\"C1\""
            } else {
                ""
            };
            let stated: String = attributes
                .iter()
                .map(|attribute| format!(" {attribute}=\"x\""))
                .collect();
            reference_block.push_str(&format!("    <{interface}{uid}{stated}/>\n"));
        }
        reference_block.push_str("  </PIDPipingComponent>\n");
        let mut generated_block = String::from("  <PIDPipingComponent>\n");
        for interface in elements.keys() {
            let uid = if *interface == "IObject" {
                " UID=\"C1\""
            } else {
                ""
            };
            generated_block.push_str(&format!("    <{interface}{uid}/>\n"));
        }
        generated_block.push_str("  </PIDPipingComponent>\n");
        let reference = document("", &reference_block);
        let generated = document("", &generated_block);

        let gate = diff_pid_export(&generated, &reference).expect("well-formed");
        assert!(gate.is_clean(), "{gate}");
        assert_eq!(gate.values_not_in_generated, 0);
        assert_eq!(gate.values_excused_by_design, excused);

        let measured = measure(&generated, &reference).expect("well-formed");
        assert_eq!(
            measured.values,
            Share {
                stated: 0,
                reference: excused
            }
        );
        assert_eq!(measured.values_by_attribute.len(), excused);
        assert_eq!(measured.interfaces.stated, measured.interfaces.reference);
    }

    #[test]
    fn a_share_prints_its_counts_and_never_rounds_up_to_the_whole() {
        let share = |stated, reference| Share { stated, reference }.to_string();
        assert_eq!(share(3323, 6702), "3323 / 6702 (49.6%)");
        assert_eq!(share(12, 12), "12 / 12 (100.0%)");
        assert_eq!(share(9999, 10_000), "9999 / 10000 (99.9%)");
        assert_eq!(share(0, 15), "0 / 15 (0.0%)");
        assert_eq!(share(0, 0), "0 / 0");
    }

    #[test]
    fn a_document_cut_short_is_refused_with_its_side() {
        let whole = reference();
        let cut = &whole[..whole.len() / 2];
        let refused = measure(cut, &whole).expect_err("the generated document is cut");
        assert_eq!(refused.side, PublishDocumentSide::Generated);
        let refused = measure(&whole, cut).expect_err("the reference is cut");
        assert_eq!(refused.side, PublishDocumentSide::Reference);
    }
}
