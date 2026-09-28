//! Parser for the existing compact constraint notation.

use std::collections::{BTreeMap, HashMap};

use crate::{
    problem::{Configuration, Constraint, DegreeGroup, LabelId, Part, PlainProblem},
    protocol::{ApiError, Location, Side},
};

pub fn parse_problem(active: &str, passive: &str) -> Result<PlainProblem, ApiError> {
    let mut parser = Parser::default();
    let active = parser.parse_constraint(active, Side::Active)?;
    let passive = parser.parse_constraint(passive, Side::Passive)?;
    Ok(PlainProblem {
        labels: parser.labels,
        active,
        passive,
    })
}

#[derive(Default)]
struct Parser {
    labels: Vec<String>,
    ids: HashMap<String, LabelId>,
}

impl Parser {
    fn parse_constraint(
        &mut self,
        text: &str,
        side: Side,
    ) -> Result<Constraint<LabelId>, ApiError> {
        let mut degrees: BTreeMap<u32, Vec<Configuration<LabelId>>> = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let location = Location {
                side,
                line: index + 1,
            };
            let (degree, configuration) = self.parse_configuration(line, location)?;
            degrees.entry(degree).or_default().push(configuration);
        }
        if degrees.is_empty() {
            return Err(ApiError::parse(
                "Enter at least one configuration.",
                side,
                1,
            ));
        }
        Ok(Constraint {
            degrees: degrees
                .into_iter()
                .map(|(degree, configurations)| DegreeGroup {
                    degree,
                    configurations,
                })
                .collect(),
        })
    }

    fn parse_configuration(
        &mut self,
        line: &str,
        location: Location,
    ) -> Result<(u32, Configuration<LabelId>), ApiError> {
        let mut parts = Vec::new();
        let mut degree = 0u32;
        for token in line.split_whitespace() {
            let part = self.parse_part(token, location)?;
            degree = degree.checked_add(part.multiplicity).ok_or_else(|| {
                ApiError::parse(
                    "Configuration degree is too large.",
                    location.side,
                    location.line,
                )
            })?;
            parts.push(part);
        }
        Ok((degree, Configuration { parts }))
    }

    fn parse_part(&mut self, token: &str, location: Location) -> Result<Part<LabelId>, ApiError> {
        let mut chars = token.chars();
        let mut names = Vec::new();
        let mut multiplicity = 1;
        while let Some(character) = chars.next() {
            match character {
                '(' => {
                    let mut name = String::new();
                    let mut closed = false;
                    for next in chars.by_ref() {
                        if next == ')' {
                            closed = true;
                            break;
                        }
                        if matches!(next, '(' | '^' | '*') {
                            return Err(ApiError::parse(
                                "Invalid character inside a parenthesized label.",
                                location.side,
                                location.line,
                            ));
                        }
                        name.push(next);
                    }
                    if name.is_empty() || !closed {
                        return Err(ApiError::parse(
                            "A parenthesized label must be nonempty and closed.",
                            location.side,
                            location.line,
                        ));
                    }
                    names.push(name);
                }
                ')' => {
                    return Err(ApiError::parse(
                        "Unexpected closing parenthesis.",
                        location.side,
                        location.line,
                    ));
                }
                '^' => {
                    let digits: String = chars.collect();
                    let count = digits.parse::<u32>().map_err(|_| {
                        ApiError::parse(
                            "Expected a positive integer after '^'.",
                            location.side,
                            location.line,
                        )
                    })?;
                    if count == 0 {
                        return Err(ApiError::parse(
                            "Repetition must be positive.",
                            location.side,
                            location.line,
                        ));
                    }
                    multiplicity = count;
                    break;
                }
                '*' => {
                    return Err(ApiError::parse(
                        "Starred configurations are not supported.",
                        location.side,
                        location.line,
                    ));
                }
                _ => names.push(character.to_string()),
            }
        }
        if names.is_empty() {
            return Err(ApiError::parse(
                "A part needs at least one label.",
                location.side,
                location.line,
            ));
        }
        let mut labels = Vec::new();
        for name in names {
            let id = self.intern(name, location)?;
            if !labels.contains(&id) {
                labels.push(id);
            }
        }
        Ok(Part {
            labels,
            multiplicity,
        })
    }

    fn intern(&mut self, name: String, location: Location) -> Result<LabelId, ApiError> {
        if let Some(&id) = self.ids.get(&name) {
            return Ok(id);
        }
        let id = LabelId(u32::try_from(self.labels.len()).map_err(|_| {
            ApiError::parse("Too many distinct labels.", location.side, location.line)
        })?);
        self.labels.push(name.clone());
        self.ids.insert(name, id);
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_existing_notation_and_groups_degrees() {
        let problem = parse_problem("M U^9\nP^10\n(foo) A", "M UP^9\nU^10").unwrap();
        assert_eq!(problem.labels, ["M", "U", "P", "foo", "A"]);
        assert_eq!(problem.active.degrees.len(), 2);
        assert_eq!(problem.active.degrees[0].degree, 2);
        assert_eq!(problem.active.degrees[1].degree, 10);
        assert_eq!(problem.active.degrees[1].configurations.len(), 2);
        let choices = &problem.passive.degrees[0].configurations[0].parts[1];
        assert_eq!(choices.labels, [LabelId(1), LabelId(2)]);
        assert_eq!(choices.multiplicity, 9);
    }

    #[test]
    fn shares_label_ids_across_sides() {
        let problem = parse_problem("A (long)A^2", "(long) A^").unwrap_err();
        assert_eq!(problem.location.unwrap().side, Side::Passive);
        let problem = parse_problem("A (long)A^2", "(long) A").unwrap();
        assert_eq!(problem.labels, ["A", "long"]);
        assert_eq!(
            problem.passive.degrees[0].configurations[0].parts[0].labels,
            [LabelId(1)]
        );
    }

    #[test]
    fn reports_unsupported_star_and_missing_parenthesis() {
        let error = parse_problem("A*", "A").unwrap_err();
        assert!(error.message.contains("not supported"));
        assert_eq!(error.location.unwrap().line, 1);
        let error = parse_problem("A", "(missing").unwrap_err();
        assert!(error.message.contains("closed"));
        assert_eq!(error.location.unwrap().side, Side::Passive);
    }

    #[test]
    fn supports_different_degrees_on_each_side() {
        let problem = parse_problem("A\nA^3", "A^2\nA^").unwrap_err();
        assert_eq!(problem.location.unwrap().side, Side::Passive);
        let problem = parse_problem("A\nA^3", "A^2").unwrap();
        assert_eq!(problem.active.degrees.len(), 2);
        assert_eq!(problem.passive.degrees[0].degree, 2);
    }
}
