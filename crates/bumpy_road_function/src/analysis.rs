//! Pure helpers for detecting "bumpy road" intervals in a smoothed signal.
//!
//! The lint pass uses these helpers after constructing and smoothing the
//! per-line complexity signal. Keeping this logic independent from `rustc_*`
//! APIs allows unit and behavioural testing without compiling the compiler
//! driver.

/// Weighting applied to signal segments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    /// Contribution added per nesting depth level.
    pub depth: f64,
    /// Contribution added per predicate branch.
    pub predicate: f64,
    /// Contribution added per control-flow construct (e.g. match arms).
    pub flow: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            depth: 1.0,
            predicate: 0.5,
            flow: 0.5,
        }
    }
}

/// User-facing configuration after normalisation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Smoothed value threshold at which a bump is considered active.
    pub threshold: f64,
    /// Centred moving-average window size.
    pub window: usize,
    /// Minimum number of contiguous lines required to keep a bump.
    pub min_bump_lines: usize,
    /// Segment weights.
    pub weights: Weights,
    /// Whether closure bodies are inspected as additional function-like scopes.
    pub include_closures: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            threshold: 3.0,
            window: 3,
            min_bump_lines: 2,
            weights: Weights::default(),
            include_closures: false,
        }
    }
}

/// Normalises settings so invalid values fall back to safe defaults.
///
/// The lint treats invalid values as configuration mistakes and falls back to
/// defaults rather than panicking or emitting spurious diagnostics.
#[must_use]
pub fn normalise_settings(settings: Settings) -> Settings {
    fn normalise_weight(candidate: f64, fallback: f64) -> f64 {
        if candidate.is_finite() && candidate >= 0.0 {
            candidate
        } else {
            fallback
        }
    }

    let defaults = Settings::default();
    let threshold = if settings.threshold.is_finite() && settings.threshold >= 0.0 {
        settings.threshold
    } else {
        defaults.threshold
    };

    let mut window = if settings.window == 0 {
        defaults.window
    } else {
        settings.window
    };
    if (window & 1) == 0 {
        window = defaults.window;
    }

    let min_bump_lines = settings.min_bump_lines.max(1);
    let weights = Weights {
        depth: normalise_weight(settings.weights.depth, defaults.weights.depth),
        predicate: normalise_weight(settings.weights.predicate, defaults.weights.predicate),
        flow: normalise_weight(settings.weights.flow, defaults.weights.flow),
    };

    Settings {
        threshold,
        window,
        min_bump_lines,
        weights,
        include_closures: settings.include_closures,
    }
}

/// A contiguous bump interval detected in a smoothed signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BumpInterval {
    start_index: usize,
    end_index: usize,
    area_above_threshold: f64,
}

impl BumpInterval {
    /// First index covered by the bump (inclusive).
    #[must_use]
    pub const fn start_index(self) -> usize {
        self.start_index
    }

    /// Last index covered by the bump (inclusive).
    #[must_use]
    pub const fn end_index(self) -> usize {
        self.end_index
    }

    /// Number of samples spanned by the bump.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end_index - self.start_index + 1
    }

    /// Returns `true` when the interval contains no samples.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start_index > self.end_index
    }

    /// Area above the threshold used for ranking bumps.
    #[must_use]
    pub const fn area_above_threshold(self) -> f64 {
        self.area_above_threshold
    }
}

/// Detects bump intervals in the smoothed signal.
///
/// A bump is a contiguous run of samples whose value is at least `threshold`.
/// Bumps shorter than `min_bump_lines` are discarded.
#[must_use]
pub fn detect_bumps(smoothed: &[f64], threshold: f64, min_bump_lines: usize) -> Vec<BumpInterval> {
    if smoothed.is_empty() {
        return Vec::new();
    }

    let min_bump_lines = min_bump_lines.max(1);
    let mut intervals = Vec::new();
    let mut current_start: Option<usize> = None;
    let mut area = 0.0_f64;

    for (index, &value) in smoothed.iter().enumerate() {
        process_sample_value(
            value,
            threshold,
            index,
            &mut current_start,
            &mut area,
            &mut intervals,
            min_bump_lines,
        );
    }

    finalize_pending_bump(
        current_start,
        smoothed.len() - 1,
        area,
        min_bump_lines,
        &mut intervals,
    );

    intervals
}

fn process_sample_value(
    value: f64,
    threshold: f64,
    index: usize,
    current_start: &mut Option<usize>,
    area: &mut f64,
    intervals: &mut Vec<BumpInterval>,
    min_bump_lines: usize,
) {
    if value >= threshold {
        start_bump_if_needed(index, current_start, area);
        *area += value - threshold;
    } else {
        finalize_current_bump(index, current_start, *area, intervals, min_bump_lines);
    }
}

fn start_bump_if_needed(index: usize, current_start: &mut Option<usize>, area: &mut f64) {
    if current_start.is_none() {
        *current_start = Some(index);
        *area = 0.0;
    }
}

fn finalize_current_bump(
    index: usize,
    current_start: &mut Option<usize>,
    area: f64,
    intervals: &mut Vec<BumpInterval>,
    min_bump_lines: usize,
) {
    let Some(start) = current_start.take() else {
        return;
    };

    let end = index.saturating_sub(1);
    if let Some(interval) = finalize_bump(start, end, area, min_bump_lines) {
        intervals.push(interval);
    }
}

fn finalize_pending_bump(
    current_start: Option<usize>,
    end: usize,
    area: f64,
    min_bump_lines: usize,
    intervals: &mut Vec<BumpInterval>,
) {
    let Some(start) = current_start else {
        return;
    };

    if let Some(interval) = finalize_bump(start, end, area, min_bump_lines) {
        intervals.push(interval);
    }
}

fn finalize_bump(
    start: usize,
    end: usize,
    area: f64,
    min_bump_lines: usize,
) -> Option<BumpInterval> {
    let interval = BumpInterval {
        start_index: start,
        end_index: end,
        area_above_threshold: area,
    };

    if interval.len() >= min_bump_lines {
        Some(interval)
    } else {
        None
    }
}

/// Returns the two most severe bumps by area (breaking ties by longest interval, then earliest).
#[must_use]
pub fn top_two_bumps(mut intervals: Vec<BumpInterval>) -> Vec<BumpInterval> {
    intervals.sort_by(|left, right| {
        right
            .area_above_threshold
            .total_cmp(&left.area_above_threshold)
            .then_with(|| right.len().cmp(&left.len()))
            .then_with(|| left.start_index.cmp(&right.start_index))
    });
    intervals.truncate(2);
    intervals
}
