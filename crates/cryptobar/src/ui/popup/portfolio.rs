use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation};

use crate::model::{Currency, Quote};
use crate::portfolio::{self, Portfolio};
use crate::render::format;

use super::identity::css_key;
use super::model::Ctx;
use super::parse::rgb;
use super::widgets::{clear, trend_class};

impl Ctx {
    /// Portfolio card: total, split, positions and watched addresses.
    pub(super) fn draw_wallet(self: &Rc<Self>, currency: &Currency, decimals: format::Decimals) {
        clear(&self.widgets.wallet);
        let model = self.model.borrow();
        let wallets = model.app.config.wallets.clone();
        let balances = model.balances.clone();
        let wallet = portfolio::compute(&model.quotes, &model.onchain);
        let quotes = model.quotes.clone();
        drop(model);

        let card = GtkBox::new(Orientation::Vertical, 14);
        card.add_css_class("card");

        let head = GtkBox::new(Orientation::Horizontal, 8);
        let title = Label::new(Some("PORTFOLIO"));
        title.add_css_class("title");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        head.append(&title);
        if let Some(pct) = wallet.as_ref().and_then(|wallet| wallet.change_24h_pct) {
            let chip = Label::new(Some(&format::percent(Some(pct))));
            chip.add_css_class("chip");
            chip.add_css_class(trend_class(Some(pct)));
            head.append(&chip);
        }
        card.append(&head);

        card.append(&wallet_hero(
            wallet.as_ref(),
            wallets.len(),
            currency,
            decimals,
        ));

        if let Some(wallet) = wallet.as_ref() {
            let positions = GtkBox::new(Orientation::Vertical, 0);
            positions.add_css_class("inset");
            for position in &wallet.positions {
                positions.append(&position_row(position, currency, decimals));
            }
            card.append(&positions);
        }

        card.append(&self.wallets_section(&wallets, &balances, &quotes, currency, decimals));
        self.widgets.wallet.append(&card);
    }

    /// Watched addresses section: one row per address, and the form to add
    /// another one.
    fn wallets_section(
        self: &Rc<Self>,
        wallets: &[crate::onchain::Wallet],
        balances: &[crate::onchain::Balance],
        quotes: &[Quote],
        currency: &Currency,
        decimals: format::Decimals,
    ) -> GtkBox {
        let section = GtkBox::new(Orientation::Vertical, 8);

        let title = Label::new(Some("WATCHED ADDRESSES"));
        title.add_css_class("subtitle");
        title.set_xalign(0.0);
        section.append(&title);

        // Without this sentence, "public address" gets confused with "key". So
        // we say where to find it, and what cryptobar refuses to accept.
        let help = Label::new(Some(
            "Paste the public address of your wallet — the one you hand out to receive \
             funds (Ledger Live: \"Receive\"). cryptobar reads the balance from the chain, \
             and can never spend. A private key or a recovery phrase is rejected and never \
             written to disk.",
        ));
        help.add_css_class("muted");
        help.set_wrap(true);
        help.set_xalign(0.0);
        section.append(&help);

        for (index, wallet) in wallets.iter().enumerate() {
            let row = GtkBox::new(Orientation::Horizontal, 10);
            row.add_css_class("wrow");

            let dot = Label::new(Some("●"));
            dot.add_css_class("dot");
            dot.add_css_class(&format!("dot-{}", css_key(wallet.chain.symbol())));
            row.append(&dot);

            let names = GtkBox::new(Orientation::Vertical, 0);
            let label = Label::new(Some(&wallet.title()));
            label.set_xalign(0.0);
            names.append(&label);
            let address = Label::new(Some(&crate::onchain::shorten(&wallet.address)));
            address.add_css_class("mono");
            address.set_xalign(0.0);
            names.append(&address);
            names.set_hexpand(true);
            row.append(&names);

            let balance = balances
                .iter()
                .find(|balance| balance.address == wallet.address);
            let amounts = GtkBox::new(Orientation::Vertical, 0);
            let quantity = Label::new(Some(&match balance {
                Some(balance) => format!(
                    "{} {}",
                    format::quantity(balance.amount),
                    wallet.chain.symbol()
                ),
                None => "reading…".to_owned(),
            }));
            quantity.set_xalign(1.0);
            amounts.append(&quantity);
            let value = quotes
                .iter()
                .find(|quote| quote.id == wallet.chain.coin())
                .zip(balance)
                .map(|(quote, balance)| quote.price * balance.amount);
            let money = Label::new(Some(&match value {
                Some(value) => format::price(value, currency, decimals),
                None => "—".to_owned(),
            }));
            money.add_css_class("muted");
            money.set_xalign(1.0);
            amounts.append(&money);
            row.append(&amounts);

            let remove = Button::with_label("✕");
            remove.add_css_class("ghost");
            remove.set_valign(Align::Center);
            remove.set_tooltip_text(Some("Stop watching this address"));
            let ctx = Rc::clone(self);
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_wallet(index);
                ctx.adopt(written, "address removed");
            });
            row.append(&remove);
            section.append(&row);
        }

        section.append(&self.wallet_form());
        section
    }

    /// Sample address for a chain, shown as a placeholder in the field.
    fn address_placeholder(chain: crate::onchain::Chain) -> &'static str {
        match chain {
            crate::onchain::Chain::Bitcoin => "bc1… or 1… or 3…",
            crate::onchain::Chain::Ethereum => "0x…",
            crate::onchain::Chain::Solana => "Solana address, e.g. 9xQe…",
        }
    }

    /// Form to add a watched address.
    fn wallet_form(self: &Rc<Self>) -> GtkBox {
        let form = GtkBox::new(Orientation::Horizontal, 8);

        let labels: Vec<&str> = crate::onchain::Chain::ALL
            .iter()
            .map(|chain| chain.symbol())
            .collect();
        let picker = gtk4::DropDown::from_strings(&labels);
        picker.set_valign(Align::Center);
        form.append(&picker);

        let address = gtk4::Entry::new();
        address.set_placeholder_text(Some(Self::address_placeholder(
            crate::onchain::Chain::Bitcoin,
        )));
        address.set_hexpand(true);
        form.append(&address);

        // The placeholder follows the chosen chain: an Ethereum address looks
        // nothing like a Bitcoin one.
        let hinted = address.clone();
        picker.connect_selected_notify(move |picker| {
            let index = usize::try_from(picker.selected()).unwrap_or(0);
            if let Some(chain) = crate::onchain::Chain::ALL.get(index).copied() {
                hinted.set_placeholder_text(Some(Self::address_placeholder(chain)));
            }
        });

        let label = gtk4::Entry::new();
        label.set_placeholder_text(Some("name, e.g. Ledger"));
        label.set_width_chars(10);
        form.append(&label);

        let add = Button::with_label("Watch");
        add.add_css_class("add");
        add.set_valign(Align::Center);
        let ctx = Rc::clone(self);
        let fields = (address.clone(), label.clone());
        add.connect_clicked(move |_| {
            let index = usize::try_from(picker.selected()).unwrap_or(0);
            let Some(chain) = crate::onchain::Chain::ALL.get(index).copied() else {
                return;
            };
            let (address, label) = &fields;
            let text = address.text().to_string();
            let name = label.text().to_string();
            let written = ctx.app().editor().add_wallet(
                chain,
                &text,
                Some(name.as_str()).filter(|name| !name.trim().is_empty()),
            );
            if written.is_ok() {
                address.set_text("");
                label.set_text("");
            }
            ctx.adopt(written, "address watched — reading the balance…");
        });
        form.append(&add);
        form
    }
}

/// Portfolio banner: split ring, total and subtitle.
fn wallet_hero(
    wallet: Option<&Portfolio>,
    watched: usize,
    currency: &Currency,
    decimals: format::Decimals,
) -> GtkBox {
    let hero = GtkBox::new(Orientation::Horizontal, 18);

    let ring = gtk4::Overlay::new();
    ring.set_child(Some(&donut(wallet)));
    let center = Label::new(Some(&match wallet {
        Some(wallet) => format!("{}", wallet.positions.len()),
        None => "0".to_owned(),
    }));
    center.add_css_class("ring-count");
    center.set_halign(Align::Center);
    center.set_valign(Align::Center);
    ring.add_overlay(&center);
    hero.append(&ring);

    let texts = GtkBox::new(Orientation::Vertical, 2);
    texts.set_valign(Align::Center);
    let total = Label::new(Some(&format::price(
        wallet.map_or(0.0, |wallet| wallet.total),
        currency,
        decimals,
    )));
    total.add_css_class("total");
    total.set_xalign(0.0);
    texts.append(&total);

    if let Some(gain) = wallet.and_then(|wallet| wallet.change_24h) {
        let sign = if gain >= 0.0 { "+" } else { "−" };
        let line = Label::new(Some(&format!(
            "{sign}{} over 24 h",
            format::price(gain.abs(), currency, decimals)
        )));
        line.add_css_class(trend_class(wallet.and_then(|wallet| wallet.change_24h_pct)));
        line.set_xalign(0.0);
        texts.append(&line);
    }

    let counts = match (wallet.map_or(0, |wallet| wallet.positions.len()), watched) {
        (0, 0) => "Add a public address to see its balance".to_owned(),
        (0, watched) => format!("{watched} watched address(es), no balance"),
        (positions, 0) => format!("{positions} position(s)"),
        (positions, watched) => format!("{positions} position(s) · {watched} watched address(es)"),
    };
    let subtitle = Label::new(Some(&counts));
    subtitle.add_css_class("muted");
    subtitle.set_xalign(0.0);
    texts.append(&subtitle);

    hero.append(&texts);
    hero
}

/// One position row: brand badge, symbol, quantity, value, share.
fn position_row(
    position: &portfolio::Position,
    currency: &Currency,
    decimals: format::Decimals,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.add_css_class("wrow");

    let dot = Label::new(Some("●"));
    dot.add_css_class("dot");
    dot.add_css_class(&format!("dot-{}", css_key(&position.symbol)));
    row.append(&dot);

    let symbol = Label::new(Some(&position.symbol.to_uppercase()));
    symbol.add_css_class("rowsym");
    symbol.set_xalign(0.0);
    symbol.set_size_request(56, -1);
    row.append(&symbol);

    let quantity = Label::new(Some(&format::quantity(position.quantity)));
    quantity.add_css_class("muted");
    quantity.set_xalign(0.0);
    quantity.set_hexpand(true);
    row.append(&quantity);

    let value = Label::new(Some(&format::price(position.value, currency, decimals)));
    value.set_xalign(1.0);
    row.append(&value);

    let share = Label::new(Some(&format!("{:.1} %", position.share * 100.0)));
    share.add_css_class("chip");
    share.add_css_class("flat");
    share.set_width_chars(7);
    row.append(&share);
    row
}

/// Split ring of the portfolio, one slice per position.
fn donut(wallet: Option<&Portfolio>) -> gtk4::DrawingArea {
    const SIZE: i32 = 86;
    let slices: Vec<(f64, String)> = wallet
        .map(|wallet| {
            wallet
                .positions
                .iter()
                .map(|position| (position.share, position.color.clone()))
                .collect()
        })
        .unwrap_or_default();

    let area = gtk4::DrawingArea::new();
    area.set_content_width(SIZE);
    area.set_content_height(SIZE);
    area.set_valign(Align::Center);
    area.set_draw_func(move |_, ctx, width, height| {
        let size = f64::from(width.min(height));
        let center = size / 2.0;
        let radius = center - 8.0;
        ctx.set_line_width(11.0);

        // Background track: the ring exists even without a position, which
        // avoids a visual hole in the card when the portfolio is empty.
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.14);
        ctx.arc(center, center, radius, 0.0, std::f64::consts::TAU);
        let _ = ctx.stroke();

        let mut start = -std::f64::consts::FRAC_PI_2;
        for (share, color) in &slices {
            // A thin gap separates the slices: two close colours stay
            // distinguishable.
            let sweep = (share.clamp(0.0, 1.0) * std::f64::consts::TAU - 0.04).max(0.01);
            let (red, green, blue) = rgb(color);
            ctx.set_source_rgb(red, green, blue);
            ctx.set_line_cap(gtk4::cairo::LineCap::Round);
            ctx.arc(center, center, radius, start, start + sweep);
            let _ = ctx.stroke();
            start += share.clamp(0.0, 1.0) * std::f64::consts::TAU;
        }
    });
    area
}
