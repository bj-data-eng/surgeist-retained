use super::{
    error::Result,
    event::Hook,
    identity::Key,
    string::{Attribute, Class, Tag, Text, validate_element_strings},
};

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Element {
    key: Option<Key>,
    kind: Kind,
    role: Role,
    label: Option<Text>,
    classes: Vec<Class>,
    attributes: Vec<Attribute>,
    text: Option<Text>,
    hooks: Vec<Hook>,
    children: Vec<Element>,
}

impl Element {
    #[must_use]
    pub fn new(kind: Kind) -> Self {
        Self {
            key: None,
            kind,
            role: Role::Generic,
            label: None,
            classes: Vec::new(),
            attributes: Vec::new(),
            text: None,
            hooks: Vec::new(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn root() -> Self {
        Self::new(Kind::Root)
    }

    #[must_use]
    pub fn tagged(tag: Tag) -> Self {
        Self::new(Kind::Element(tag))
    }

    #[must_use]
    pub fn text(text: Text) -> Self {
        Self::new(Kind::Text).with_text(text)
    }

    #[must_use]
    pub fn fragment() -> Self {
        Self::new(Kind::Fragment)
    }

    #[must_use]
    pub fn canvas() -> Self {
        Self::new(Kind::Canvas)
    }

    #[must_use]
    pub fn widget(tag: Tag) -> Self {
        Self::new(Kind::Widget(tag))
    }

    #[must_use]
    pub fn slot(tag: Tag) -> Self {
        Self::new(Kind::Slot(tag))
    }

    #[must_use]
    pub fn with_key(mut self, key: Key) -> Self {
        self.key = Some(key);
        self
    }

    #[must_use]
    pub fn without_key(mut self) -> Self {
        self.key = None;
        self
    }

    #[must_use]
    pub fn with_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: Text) -> Self {
        self.label = Some(label);
        self
    }

    #[must_use]
    pub fn with_class(mut self, class: Class) -> Self {
        self.classes.push(class);
        self
    }

    #[must_use]
    pub fn with_attribute(mut self, attribute: Attribute) -> Self {
        if let Some(existing) = self
            .attributes
            .iter_mut()
            .find(|existing| existing.name == attribute.name)
        {
            *existing = attribute;
        } else {
            self.attributes.push(attribute);
        }
        self
    }

    #[must_use]
    pub fn with_text(mut self, text: Text) -> Self {
        self.text = Some(text);
        self
    }

    #[must_use]
    pub fn with_hook(mut self, hook: Hook) -> Self {
        self.hooks.push(hook);
        self
    }

    #[must_use]
    pub fn with_child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    #[must_use]
    pub fn with_children(mut self, children: impl IntoIterator<Item = Element>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn validate(&self) -> Result<()> {
        validate_element_strings(self)
    }

    #[must_use]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    #[must_use]
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role.clone()
    }

    #[must_use]
    pub fn label(&self) -> Option<&Text> {
        self.label.as_ref()
    }

    #[must_use]
    pub fn classes(&self) -> &[Class] {
        &self.classes
    }

    #[must_use]
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    #[must_use]
    pub fn text_content(&self) -> Option<&Text> {
        self.text.as_ref()
    }

    #[must_use]
    pub fn hooks(&self) -> &[Hook] {
        &self.hooks
    }

    #[must_use]
    pub fn children(&self) -> &[Element] {
        &self.children
    }

    pub(crate) fn take_children(&mut self) -> Vec<Element> {
        std::mem::take(&mut self.children)
    }

    pub(crate) fn set_kind(&mut self, kind: Kind) {
        self.kind = kind;
    }

    pub(crate) fn set_role(&mut self, role: Role) {
        self.role = role;
    }

    pub(crate) fn set_label(&mut self, label: Option<Text>) {
        self.label = label;
    }

    pub(crate) fn set_classes(&mut self, classes: Vec<Class>) {
        self.classes = classes;
    }

    pub(crate) fn set_attribute(&mut self, attribute: Attribute) {
        if let Some(existing) = self
            .attributes
            .iter_mut()
            .find(|existing| existing.name == attribute.name)
        {
            *existing = attribute;
        } else {
            self.attributes.push(attribute);
        }
    }

    pub(crate) fn remove_attribute(&mut self, name: &super::AttributeName) {
        self.attributes.retain(|attribute| &attribute.name != name);
    }

    pub(crate) fn set_text(&mut self, text: Option<Text>) {
        self.text = text;
    }

    pub(crate) fn set_hooks(&mut self, hooks: Vec<Hook>) {
        self.hooks = hooks;
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    Root,
    Element(Tag),
    Text,
    Canvas,
    Fragment,
    Slot(Tag),
    Widget(Tag),
}

#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Role {
    #[default]
    Generic,
    Application,
    Button,
    Text,
    List,
    ListItem,
    Checkbox,
    Textbox,
    Image,
    Canvas,
    Widget,
    Custom(Tag),
}
