//! The jobs with no end when a run stops (T-339 item 2): each open cell by
//! its name with its matrix, and each job that `--job` keeps out.

use podrunner_schema::JobId;

use crate::LocalSource;

impl LocalSource {
    /// Whether `--job` keeps the job `id` out of the run.
    pub fn left_out(&self, id: &str) -> bool {
        let caller = id.split(['/', '[']).next().unwrap_or(id);
        self.selected.as_ref().is_some_and(|s| !s.contains(id) && !s.contains(caller))
    }

    /// The cells of the job `id` that were offered and have no end: the
    /// offer and the name of each, with its matrix.
    pub fn open_cells(&self, id: &str) -> Vec<(JobId, String)> {
        self.cells
            .iter()
            .filter(|(j, c)| c.job == id && !self.cell_ends.iter().any(|(e, _, _)| e == *j))
            .map(|(j, c)| (j.clone(), c.instance.name.clone()))
            .collect()
    }
}
