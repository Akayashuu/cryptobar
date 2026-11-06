/// Base stylesheet.
///
/// Every shade is derived from `@theme_bg_color` and `@theme_fg_color` through
/// the GTK colour functions: the window matches the desktop theme, light or
/// dark, without freezing or reading it.
pub(super) const CSS: &str = "
window { background-color: @theme_bg_color; }

.card {
  background-color: mix(@theme_bg_color, @theme_fg_color, 0.05);
  border-radius: 20px;
  padding: 18px;
}
.hero { padding: 20px 22px; }

/* Tabs: flat pills, in the same family as the chart range buttons, rather
   than the default tab bar. GTK paints its buttons with a `background-image`:
   neutralising it is necessary, `background-color` alone would not do. */
.tabs { background: none; padding: 0; }
.tabs button {
  background-image: none;
  background-color: transparent;
  border: none;
  box-shadow: none;
  border-radius: 999px;
  padding: 6px 14px;
  min-height: 0;
  font-weight: 600;
  opacity: 0.5;
}
.tabs button:hover {
  background-color: alpha(@theme_fg_color, 0.06);
  opacity: 0.8;
}
.tabs button:checked {
  background-color: alpha(@theme_fg_color, 0.12);
  opacity: 1;
}

.badge {
  border-radius: 999px;
  font-weight: 800;
  font-size: 26px;
  min-width: 56px;
  min-height: 56px;
  /* Neutral fallback: a coin with no known brand colour — a search result,
     say — stays readable instead of vanishing. */
  background-color: alpha(@theme_fg_color, 0.10);
  color: @theme_fg_color;
}
.badge.small { font-size: 15px; min-width: 32px; min-height: 32px; }

.title  { font-size: 13px; font-weight: 700; letter-spacing: 2px; opacity: 0.35; }
.symbol { font-size: 24px; font-weight: 800; }
.name   { font-size: 15px; opacity: 0.55; }
.price  { font-size: 46px; font-weight: 800; }
.rowsym { font-size: 16px; font-weight: 700; }

.chip {
  border-radius: 999px;
  padding: 3px 11px;
  font-size: 13px;
  font-weight: 600;
  background-color: alpha(@theme_fg_color, 0.08);
}
.chip.up   { color: #2fbf71; background-color: alpha(#2fbf71, 0.16); }
.chip.down { color: #e5484d; background-color: alpha(#e5484d, 0.16); }
.chip.flat { opacity: 0.55; }
.chip-key  { font-size: 12px; opacity: 0.45; font-weight: 600; }

.subtitle { font-size: 11px; font-weight: 700; letter-spacing: 1.5px; opacity: 0.3; }
.total  { font-size: 30px; font-weight: 800; }
.ring-count { font-size: 18px; font-weight: 800; opacity: 0.5; }
.dot    { font-size: 11px; }
.mono   { font-family: monospace; font-size: 12px; opacity: 0.45; }

/* Recessed block: separates the lists from the card background. */
.inset {
  background-color: alpha(@theme_fg_color, 0.04);
  border-radius: 14px;
  padding: 4px 10px;
}
.wrow { padding: 6px 2px; }

expander title { font-size: 13px; opacity: 0.6; }

.up   { color: #2fbf71; }
.down { color: #e5484d; }
.flat { opacity: 0.6; }
.muted { opacity: 0.5; font-size: 13px; }
.error { color: #e5484d; font-size: 13px; }

/* Time range pills. */
.range {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 999px;
  padding: 3px 13px;
  min-height: 26px;
  font-size: 13px;
  font-weight: 600;
  opacity: 0.5;
}
.range:hover { opacity: 0.85; background-color: alpha(@theme_fg_color, 0.06); }
.range.on {
  opacity: 1;
  background-color: alpha(@theme_fg_color, 0.12);
}

.ghost {
  background: none;
  border: none;
  box-shadow: none;
  padding: 0 8px;
  min-height: 26px;
  min-width: 26px;
  opacity: 0.3;
}
.ghost:hover { opacity: 1; color: #e5484d; }

.add { padding: 2px 12px; min-height: 28px; border-radius: 999px; }

entry { border-radius: 14px; min-height: 38px; }

list { background-color: transparent; }
row { padding: 9px 12px; border-radius: 14px; }
row.active { background-color: alpha(@theme_fg_color, 0.08); }
row:hover { background-color: alpha(@theme_fg_color, 0.05); }
";
