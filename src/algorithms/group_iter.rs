use crate::{constraint::Constraint, group::Group, line::Line, part::Part};

impl Constraint {
    pub fn groups(&self) -> impl Iterator<Item = &'_ Group> {
        self.lines.iter().flat_map(|line| line.groups())
    }

    pub fn edited<T>(&self, f: T) -> Self
    where
        T: Fn(&Group) -> Group + Copy,
    {
        let mut c = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.degree,
        };
        for line in &self.lines {
            let newline = line.edited(f);
            if newline.parts.iter().all(|part| !part.group.0.is_empty()) {
                c.lines.push(newline);
            }
        }
        c
    }
}

impl Line {
    pub fn groups(&self) -> impl Iterator<Item = &'_ Group> {
        self.parts.iter().map(|part| &part.group)
    }

    pub fn edited<T>(&self, f: T) -> Self
    where
        T: Fn(&Group) -> Group + Copy,
    {
        let mut line = Line { parts: vec![] };
        for part in &self.parts {
            let newpart = part.edited(f);
            line.parts.push(newpart);
        }
        line.normalize();
        line
    }
}

impl Part {
    pub fn edited<T>(&self, f: T) -> Self
    where
        T: Fn(&Group) -> Group + Copy,
    {
        Part {
            gtype: self.gtype,
            group: f(&self.group),
        }
    }
}
