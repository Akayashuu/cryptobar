//! Tracé de courbes en caractères braille (2 colonnes × 4 lignes par glyphe).
//!
//! Le bloc braille Unicode commence à U+2800 ; chaque point allumé correspond
//! à un bit du motif. Une cellule offrant huit points, la résolution est
//! quadruplée verticalement par rapport aux caractères de bloc.

/// Correspondance (colonne, ligne) → bit du motif braille.
/// Colonne 0 = points 1,2,3,7 ; colonne 1 = points 4,5,6,8.
const DOTS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// Toile de points braille adressable en pixels.
pub struct Canvas {
    /// Largeur et hauteur en cellules.
    cols: usize,
    rows: usize,
    /// Un motif par cellule, indexé `row * cols + col`.
    cells: Vec<u8>,
}

impl Canvas {
    /// Crée une toile de `cols` × `rows` cellules, soit `2*cols` × `4*rows` points.
    #[must_use]
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![0; cols.saturating_mul(rows)],
        }
    }

    /// Largeur en points.
    #[must_use]
    pub fn width(&self) -> usize {
        self.cols.saturating_mul(2)
    }

    /// Hauteur en points.
    #[must_use]
    pub fn height(&self) -> usize {
        self.rows.saturating_mul(4)
    }

    /// Allume le point `(x, y)`, origine en haut à gauche. Hors toile : ignoré.
    pub fn set(&mut self, x: usize, y: usize) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let (col, sub_x) = (x.checked_div(2).unwrap_or(0), x.checked_rem(2).unwrap_or(0));
        let (row, sub_y) = (y.checked_div(4).unwrap_or(0), y.checked_rem(4).unwrap_or(0));
        let Some(bit) = DOTS.get(sub_x).and_then(|c| c.get(sub_y)) else {
            return;
        };
        let Some(index) = row.checked_mul(self.cols).and_then(|i| i.checked_add(col)) else {
            return;
        };
        if let Some(cell) = self.cells.get_mut(index) {
            *cell |= *bit;
        }
    }

    /// Rend la toile en lignes de texte.
    #[must_use]
    pub fn to_lines(&self) -> Vec<String> {
        (0..self.rows)
            .map(|row| {
                (0..self.cols)
                    .map(|col| {
                        let index = row.saturating_mul(self.cols).saturating_add(col);
                        let pattern = self.cells.get(index).copied().unwrap_or(0);
                        // U+2800 + motif : conversion toujours valide sur 8 bits.
                        char::from_u32(0x2800_u32.saturating_add(u32::from(pattern))).unwrap_or(' ')
                    })
                    .collect()
            })
            .collect()
    }
}

/// Conversion `usize` → `f64` pour les calculs de position. Les dimensions
/// d'un terminal tiennent très largement dans la mantisse d'un `f64` ; le cas
/// dégénéré est borné plutôt que d'être laissé imprécis.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn as_f64(value: usize) -> f64 {
    value.min(1 << 52) as f64
}

/// Conversion `f64` → `usize` d'une valeur déjà bornée dans `0..=max`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_usize(value: f64, max: usize) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    (value as usize).min(max)
}

/// Bornes verticales d'une série, élargies si la série est plate.
#[must_use]
pub fn bounds(points: &[f64]) -> (f64, f64) {
    let finite = points.iter().copied().filter(|v| v.is_finite());
    let min = finite.clone().fold(f64::INFINITY, f64::min);
    let max = finite.fold(f64::NEG_INFINITY, f64::max);
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if (max - min).abs() < f64::EPSILON {
        // Série plate : on ouvre une fenêtre arbitraire pour éviter une division
        // par zéro et centrer la ligne.
        return (min - 0.5, max + 0.5);
    }
    (min, max)
}

/// Trace une série sur une toile de `cols` × `rows` cellules.
#[must_use]
pub fn plot(points: &[f64], cols: usize, rows: usize) -> Vec<String> {
    let mut canvas = Canvas::new(cols, rows);
    if points.is_empty() || cols == 0 || rows == 0 {
        return canvas.to_lines();
    }
    let (min, max) = bounds(points);
    let span = max - min;
    let width = canvas.width();
    let height = canvas.height();
    let last_x = width.saturating_sub(1);
    let last_y = height.saturating_sub(1);

    let mut previous: Option<(usize, usize)> = None;
    for x in 0..width {
        // Position fractionnaire dans la série pour cette colonne de points.
        let ratio = if last_x == 0 {
            0.0
        } else {
            as_f64(x) / as_f64(last_x)
        };
        let position = ratio * as_f64(points.len().saturating_sub(1));
        let index = as_usize(position.round(), points.len().saturating_sub(1));
        let Some(value) = points.get(index) else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        let normalized = ((value - min) / span).clamp(0.0, 1.0);
        // L'axe des ordonnées est inversé : la valeur haute est en haut.
        let y = as_usize(((1.0 - normalized) * as_f64(last_y)).round(), last_y);
        if let Some((prev_x, prev_y)) = previous {
            connect(&mut canvas, prev_x, prev_y, x, y);
        }
        canvas.set(x, y);
        previous = Some((x, y));
    }
    canvas.to_lines()
}

/// Relie verticalement deux points consécutifs pour éviter une courbe pointillée.
fn connect(canvas: &mut Canvas, _prev_x: usize, prev_y: usize, x: usize, y: usize) {
    let (low, high) = if prev_y <= y {
        (prev_y, y)
    } else {
        (y, prev_y)
    };
    for step in low..=high {
        canvas.set(x, step);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn une_toile_vide_ne_contient_que_des_braille_blancs() {
        let lines = Canvas::new(3, 2).to_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.chars().all(|c| c == '\u{2800}')));
    }

    #[test]
    fn le_premier_point_correspond_au_bit_attendu() {
        let mut canvas = Canvas::new(1, 1);
        canvas.set(0, 0);
        assert_eq!(canvas.to_lines().first().unwrap(), "\u{2801}");
    }

    #[test]
    fn les_huit_points_dune_cellule_donnent_le_glyphe_plein() {
        let mut canvas = Canvas::new(1, 1);
        for x in 0..2 {
            for y in 0..4 {
                canvas.set(x, y);
            }
        }
        assert_eq!(canvas.to_lines().first().unwrap(), "\u{28ff}");
    }

    #[test]
    fn un_point_hors_toile_est_ignore_sans_panic() {
        let mut canvas = Canvas::new(1, 1);
        canvas.set(99, 99);
        assert_eq!(canvas.to_lines().first().unwrap(), "\u{2800}");
    }

    #[test]
    fn les_dimensions_du_trace_respectent_la_demande() {
        let lines = plot(&[1.0, 2.0, 3.0], 10, 4);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| l.chars().count() == 10));
    }

    #[test]
    fn une_serie_croissante_monte_de_gauche_a_droite() {
        let lines = plot(&(0..50).map(f64::from).collect::<Vec<_>>(), 20, 4);
        let first = lines.first().unwrap();
        let last = lines.last().unwrap();
        // Le haut du graphique est rempli à droite, le bas à gauche.
        assert_ne!(last.chars().next(), Some('\u{2800}'));
        assert_ne!(first.chars().last(), Some('\u{2800}'));
    }

    #[test]
    fn une_serie_plate_ne_divise_pas_par_zero() {
        let lines = plot(&[5.0; 20], 10, 3);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn les_valeurs_non_finies_sont_ignorees() {
        let lines = plot(&[1.0, f64::NAN, 3.0, f64::INFINITY], 8, 3);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn bounds_elargit_une_serie_plate() {
        let (min, max) = bounds(&[2.0, 2.0]);
        assert!(max > min);
    }

    #[test]
    fn bounds_retombe_sur_un_intervalle_sur_une_serie_inutilisable() {
        assert_eq!(bounds(&[f64::NAN]), (0.0, 1.0));
    }

    #[test]
    fn une_serie_vide_ou_une_toile_nulle_ne_paniquent_pas() {
        assert_eq!(plot(&[], 10, 3).len(), 3);
        assert!(plot(&[1.0, 2.0], 0, 0).is_empty());
    }
}
