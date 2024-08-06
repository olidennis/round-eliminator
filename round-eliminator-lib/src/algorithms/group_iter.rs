use crate::{constraint::Constraint, group::Group, line::Line, part::Part};

impl Constraint {
    pub fn groups(&self) -> impl Iterator<Item = &'_ Group> {
        self.lines.iter().flat_map(|line| line.groups())
    }

    pub fn edited<T>(&self, mut f: T) -> Self
    where
        T: FnMut(&Group) -> Group,
    {
        let mut c = Constraint {
            lines: vec![],
            is_maximized: false,
            degree: self.degree,
        };
        for line in &self.lines {
            let newline = line.edited(&mut f);
            if newline.parts.iter().all(|part| !part.group.is_empty()) {
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

    pub fn edited<T>(&self, mut f: T) -> Self
    where
        T: FnMut(&Group) -> Group,
    {
        let mut line = Line { parts: vec![] };
        for part in &self.parts {
            let newpart = part.edited(&mut f);
            line.parts.push(newpart);
        }
        line.normalize();
        line
    }
}

impl Part {
    pub fn edited<T>(&self, mut f: T) -> Self
    where
        T: FnMut(&Group) -> Group,
    {
        Part {
            gtype: self.gtype,
            group: f(&self.group),
        }
    }
}
