// SPDX-License-Identifier: Apache-2.0

#![allow(rustdoc::invalid_html_tags)]

//! A group specification.

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::attribute::{AttributeSpec, AttributeType, PrimitiveOrArrayTypeSpec};
use crate::group::InstrumentSpec::{Counter, Gauge, Histogram, UpDownCounter};
use crate::stability::Stability;
use crate::Error;
use weaver_common::error::handle_errors;

/// Group Spec contain the list of semantic conventions for attributes,
/// metrics, events, spans, etc.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupSpec {
    /// The id that uniquely identifies the semantic convention.
    pub id: String,
    /// The type of the semantic convention (default to span).
    #[serde(default)]
    pub r#type: GroupType,
    /// A brief description of the semantic convention.
    pub brief: String,
    /// A more elaborate description of the semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    pub note: String,
    /// Prefix for the attributes for this semantic convention.
    /// It defaults to an empty string.
    #[serde(default)]
    pub prefix: String,
    /// Reference another semantic convention id. It inherits the prefix,
    /// constraints, and all attributes defined in the specified semantic
    /// convention.
    pub extends: Option<String>,
    /// Specifies the stability of the semantic convention.
    /// Note that, if stability is missing but deprecated is present, it will
    /// automatically set the stability to deprecated. If deprecated is
    /// present and stability differs from deprecated, this will result in an
    /// error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Specifies if the semantic convention is deprecated. The string
    /// provided as <description> MUST specify why it's deprecated and/or what
    /// to use instead. See also stability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// List of attributes that belong to the semantic convention.
    #[serde(default)]
    pub attributes: Vec<AttributeSpec>,
    /// Additional constraints.
    /// Allow to define additional requirements on the semantic convention.
    /// It defaults to an empty list.
    #[serde(default)]
    pub constraints: Vec<ConstraintSpec>,
    /// Specifies the kind of the span.
    /// Note: only valid if type is span (the default)
    pub span_kind: Option<SpanKindSpec>,
    /// List of strings that specify the ids of event semantic conventions
    /// associated with this span semantic convention.
    /// Note: only valid if type is span (the default)
    #[serde(default)]
    pub events: Vec<String>,
    /// The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
    /// Note: This field is required if type is metric.
    pub metric_name: Option<String>,
    /// The instrument type that should be used to record the metric. Note that
    /// the semantic conventions must be written using the names of the
    /// synchronous instrument types (counter, gauge, updowncounter and
    /// histogram).
    /// For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
    /// Note: This field is required if type is metric.
    pub instrument: Option<InstrumentSpec>,
    /// The unit in which the metric is measured, which should adhere to the
    /// [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).
    /// Note: This field is required if type is metric.
    pub unit: Option<String>,
    /// The name of the event. If not specified, the prefix is used.
    /// If prefix is empty (or unspecified), name is required.
    pub name: Option<String>,
}

impl GroupSpec {
    /// Validation logic for the group.
    pub(crate) fn validate(&self, path_or_url: &str) -> Result<(), Error> {
        let mut errors = vec![];

        // Fields span_kind and events are only valid if type is span (the default).
        if self.r#type != GroupType::Span {
            if self.span_kind.is_some() {
                errors.push(Error::InvalidGroup {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a span_kind field but the type is not set to span."
                        .to_owned(),
                });
            }
            if !self.events.is_empty() {
                errors.push(Error::InvalidGroup {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains an events field but the type is not set to span."
                        .to_owned(),
                });
            }
        }

        // Field name is required if prefix is empty and if type is event.
        if self.r#type == GroupType::Event && self.prefix.is_empty() && self.name.is_none() {
            errors.push(Error::InvalidGroup {
                path_or_url: path_or_url.to_owned(),
                group_id: self.id.clone(),
                error: "This group contains an event type but the prefix is empty and the name is not set.".to_owned(),
            });
        }

        // Fields metric_name, instrument and unit are required if type is metric.
        if self.r#type == GroupType::Metric {
            if self.metric_name.is_none() {
                errors.push(Error::InvalidMetric {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a metric type but the metric_name is not set."
                        .to_owned(),
                });
            }
            if self.instrument.is_none() {
                errors.push(Error::InvalidMetric {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a metric type but the instrument is not set."
                        .to_owned(),
                });
            }
            if self.unit.is_none() {
                errors.push(Error::InvalidMetric {
                    path_or_url: path_or_url.to_owned(),
                    group_id: self.id.clone(),
                    error: "This group contains a metric type but the unit is not set.".to_owned(),
                });
            }
        }

        // Validates the attributes.
        for attribute in &self.attributes {
            // If deprecated is present and stability differs from deprecated, this
            // will result in an error.
            match attribute {
                AttributeSpec::Id {
                    brief, deprecated, ..
                } => {
                    if brief.is_none() && deprecated.is_none() {
                        errors.push(Error::InvalidAttribute {
                            path_or_url: path_or_url.to_owned(),
                            group_id: self.id.clone(),
                            attribute_id: attribute.id(),
                            error: "This attribute is not deprecated and does not contain a brief field.".to_owned(),
                        });
                    }
                }
                AttributeSpec::Ref { .. } => {}
            }

            // Examples are required only for string and string array attributes.
            if let AttributeSpec::Id {
                r#type, examples, ..
            } = attribute
            {
                if examples.is_some() {
                    continue;
                }

                if *r#type == AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String) {
                    errors.push(Error::InvalidAttribute {
                        path_or_url: path_or_url.to_owned(),
                        group_id: self.id.clone(),
                        attribute_id: attribute.id(),
                        error: "This attribute is a string but it does not contain any examples."
                            .to_owned(),
                    });
                }
                if *r#type == AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings) {
                    errors.push(Error::InvalidAttribute {
                        path_or_url: path_or_url.to_owned(),
                        group_id: self.id.clone(),
                        attribute_id: attribute.id(),
                        error:
                            "This attribute is a string array but it does not contain any examples."
                                .to_owned(),
                    });
                }
            }
        }

        handle_errors(errors)?;

        Ok(())
    }
}

/// The different types of groups (specification).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash, Clone)]
#[serde(rename_all = "snake_case")]
pub enum GroupType {
    /// Attribute group (attribute_group type) defines a set of attributes that
    /// can be declared once and referenced by semantic conventions for
    /// different signals, for example spans and logs. Attribute groups don't
    /// have any specific fields and follow the general semconv semantics.
    AttributeGroup,
    /// Span semantic convention.
    Span,
    /// Event semantic convention.
    Event,
    /// Metric semantic convention.
    Metric,
    /// The metric group semconv is a group where related metric attributes can
    /// be defined and then referenced from other metric groups using ref.
    MetricGroup,
    /// A group of resources.
    Resource,
    /// Scope.
    Scope,
}

impl Default for GroupType {
    /// Returns the default convention type that is span based on
    /// the OpenTelemetry specification.
    fn default() -> Self {
        Self::Span
    }
}

/// The span kind.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SpanKindSpec {
    /// An internal span.
    Internal,
    /// A client span.
    Client,
    /// A server span.
    Server,
    /// A producer span.
    Producer,
    /// A consumer span.
    Consumer,
}

/// Allow to define additional requirements on the semantic convention.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConstraintSpec {
    /// any_of accepts a list of sequences. Each sequence contains a list of
    /// attribute ids that are required. any_of enforces that all attributes
    /// of at least one of the sequences are set.
    #[serde(default)]
    pub any_of: Vec<String>,
    /// include accepts a semantic conventions id. It includes as part of this
    /// semantic convention all constraints and required attributes that are
    /// not already defined in the current semantic convention.
    pub include: Option<String>,
}

/// The type of the metric.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentSpec {
    /// An up-down counter metric.
    #[serde(rename = "updowncounter")]
    UpDownCounter,
    /// A counter metric.
    Counter,
    /// A gauge metric.
    Gauge,
    /// A histogram metric.
    Histogram,
}

/// Implements a human readable display for the instrument.
impl Display for InstrumentSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UpDownCounter => write!(f, "updowncounter"),
            Counter => write!(f, "counter"),
            Gauge => write!(f, "gauge"),
            Histogram => write!(f, "histogram"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute::Examples;
    use crate::Error::{CompoundError, InvalidAttribute, InvalidGroup, InvalidMetric};

    use super::*;

    #[test]
    fn test_validate_group() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Span,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "test".to_owned(),
            extends: None,
            stability: Some(Stability::Deprecated),
            deprecated: Some("true".to_owned()),
            attributes: vec![AttributeSpec::Id {
                id: "test".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: None,
                stability: Some(Stability::Deprecated),
                deprecated: Some("true".to_owned()),
                examples: Some(Examples::String("test".to_owned())),
                tag: None,
                requirement_level: Default::default(),
                sampling_relevant: None,
                note: "".to_owned(),
            }],
            constraints: vec![],
            span_kind: Some(SpanKindSpec::Client),
            events: vec!["event".to_owned()],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
        };
        assert!(group.validate("<test>").is_ok());

        // Span kind is set but the type is not span.
        group.r#type = GroupType::Metric;
        let result = group.validate("<test>");
        assert_eq!(
            Err(CompoundError(vec![
                InvalidGroup {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a span_kind field but the type is not set to span."
                        .to_owned(),
                },
                InvalidGroup {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains an events field but the type is not set to span."
                        .to_owned(),
                },
                InvalidMetric {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a metric type but the metric_name is not set."
                        .to_owned(),
                },
                InvalidMetric {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a metric type but the instrument is not set."
                        .to_owned(),
                },
                InvalidMetric {
                    path_or_url: "<test>".to_owned(),
                    group_id: "test".to_owned(),
                    error: "This group contains a metric type but the unit is not set.".to_owned(),
                },
            ],),),
            result
        );

        // Field name is required if prefix is empty and if type is event.
        group.r#type = GroupType::Event;
        "".clone_into(&mut group.prefix);
        group.name = None;
        let result = group.validate("<test>");
        assert_eq!(Err(
            CompoundError(
                vec![
                    InvalidGroup {
                        path_or_url: "<test>".to_owned(),
                        group_id: "test".to_owned(),
                        error: "This group contains a span_kind field but the type is not set to span.".to_owned(),
                    },
                    InvalidGroup {
                        path_or_url: "<test>".to_owned(),
                        group_id: "test".to_owned(),
                        error: "This group contains an events field but the type is not set to span.".to_owned(),
                    },
                    InvalidGroup {
                        path_or_url: "<test>".to_owned(),
                        group_id: "test".to_owned(),
                        error: "This group contains an event type but the prefix is empty and the name is not set.".to_owned(),
                    },
                ],
            ),
        ), result);
    }

    #[test]
    fn test_validate_attribute() {
        let mut group = GroupSpec {
            id: "test".to_owned(),
            r#type: GroupType::Span,
            brief: "test".to_owned(),
            note: "test".to_owned(),
            prefix: "test".to_owned(),
            extends: None,
            stability: Some(Stability::Deprecated),
            deprecated: Some("true".to_owned()),
            attributes: vec![AttributeSpec::Id {
                id: "test".to_owned(),
                r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
                brief: None,
                stability: Some(Stability::Deprecated),
                deprecated: Some("true".to_owned()),
                examples: Some(Examples::String("test".to_owned())),
                tag: None,
                requirement_level: Default::default(),
                sampling_relevant: None,
                note: "".to_owned(),
            }],
            constraints: vec![],
            span_kind: Some(SpanKindSpec::Client),
            events: vec!["event".to_owned()],
            metric_name: None,
            instrument: None,
            unit: None,
            name: None,
        };
        assert!(group.validate("<test>").is_ok());

        // Examples are mandatory for string attributes.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::String),
            brief: None,
            stability: Some(Stability::Deprecated),
            deprecated: Some("true".to_owned()),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
        }];
        let result = group.validate("<test>");
        assert_eq!(
            Err(InvalidAttribute {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "This attribute is a string but it does not contain any examples."
                    .to_owned(),
            },),
            result
        );

        // Examples are mandatory for strings attributes.
        group.attributes = vec![AttributeSpec::Id {
            id: "test".to_owned(),
            r#type: AttributeType::PrimitiveOrArray(PrimitiveOrArrayTypeSpec::Strings),
            brief: None,
            stability: Some(Stability::Deprecated),
            deprecated: Some("true".to_owned()),
            examples: None,
            tag: None,
            requirement_level: Default::default(),
            sampling_relevant: None,
            note: "".to_owned(),
        }];
        let result = group.validate("<test>");
        assert_eq!(
            Err(InvalidAttribute {
                path_or_url: "<test>".to_owned(),
                group_id: "test".to_owned(),
                attribute_id: "test".to_owned(),
                error: "This attribute is a string array but it does not contain any examples."
                    .to_owned(),
            },),
            result
        );
    }

    #[test]
    fn test_instrumentation_spec() {
        assert_eq!(Counter.to_string(), "counter");
        assert_eq!(Gauge.to_string(), "gauge");
        assert_eq!(Histogram.to_string(), "histogram");
        assert_eq!(UpDownCounter.to_string(), "updowncounter");
    }
}

/// A group spec with its provenance (path or URL).
#[derive(Debug, Clone)]
pub struct GroupSpecWithProvenance {
    /// The group spec.
    pub spec: GroupSpec,
    /// The provenance of the group spec (path or URL).
    pub provenance: String,
}
