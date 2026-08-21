//! Définition de l'interface en ligne de commande.

use clap::{Parser, Subcommand};

use crate::model::Range;

/// Cours de cryptomonnaies pour Waybar, le terminal et le bureau.
#[derive(Debug, Parser)]
#[command(name = "cryptobar", version, about, long_about = None)]
pub struct Cli {
    /// Chemin d'un fichier de configuration alternatif.
    #[arg(long, global = true, value_name = "FICHIER")]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Émet une ligne JSON pour un module Waybar (comportement par défaut).
    Bar {
        /// Reste en avant-plan et réémet à chaque intervalle.
        #[arg(long)]
        watch: bool,
    },
    /// Passe à la cryptomonnaie suivante.
    Next,
    /// Revient à la cryptomonnaie précédente.
    Prev,
    /// Force un rafraîchissement immédiat des données.
    Refresh,
    /// Affiche un graphique dans le terminal.
    Chart {
        /// Identifiant CoinGecko ; par défaut, la cryptomonnaie affichée.
        coin: Option<String>,
        /// Plage temporelle : 1h, 24h, 7d, 30d, 1y.
        #[arg(long, default_value = "7d")]
        range: Range,
    },
    /// Ouvre la fenêtre graphique.
    Popup,
    /// Recherche une cryptomonnaie par nom ou symbole.
    Coins {
        /// Terme recherché.
        term: String,
    },
    /// Gère le fichier de configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Écrit un fichier de configuration commenté s'il n'existe pas.
    Init {
        /// Écrase un fichier existant.
        #[arg(long)]
        force: bool,
    },
    /// Affiche le chemin du fichier de configuration.
    Path,
    /// Vérifie que la configuration est valide.
    Check,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn la_definition_clap_est_coherente() {
        Cli::command().debug_assert();
    }

    #[test]
    fn sans_sous_commande_le_champ_est_vide() {
        let cli = Cli::try_parse_from(["cryptobar"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn la_plage_du_graphique_est_validee() {
        assert!(Cli::try_parse_from(["cryptobar", "chart", "--range", "42x"]).is_err());
        let cli = Cli::try_parse_from(["cryptobar", "chart", "--range", "24h"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Chart {
                range: Range::H24,
                ..
            })
        ));
    }
}
