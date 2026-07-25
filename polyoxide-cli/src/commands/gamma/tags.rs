use clap::Subcommand;
use color_eyre::eyre::Result;
use polyoxide_gamma::Gamma;

use crate::commands::gamma::SortOrder;

#[derive(Subcommand)]
pub enum TagsCommand {
    /// List tags
    List {
        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Pagination offset
        #[arg(short, long, default_value = "0")]
        offset: u32,
        /// Sort order
        #[arg(long, value_enum, default_value = "desc")]
        sort: SortOrder,
        /// Order by field
        #[arg(long)]
        order: Option<String>,
        /// Filter by carousel status
        #[arg(long)]
        is_carousel: Option<bool>,
    },
    /// Get a tag by ID
    Get {
        /// Tag ID
        id: String,
    },
    /// Get a tag by slug
    GetBySlug {
        /// Tag slug
        slug: String,
    },
    /// Get related tags by ID
    Related {
        /// Tag ID
        id: String,
    },
    /// Get related tags by slug
    RelatedBySlug {
        /// Tag slug
        slug: String,
    },
}

impl TagsCommand {
    pub async fn run(self, gamma: &Gamma) -> Result<()> {
        match self {
            Self::List {
                limit,
                offset,
                sort,
                order,
                is_carousel,
            } => {
                let mut request = gamma
                    .tags()
                    .list()
                    .limit(limit)
                    .offset(offset)
                    .ascending(matches!(sort, SortOrder::Asc));

                if let Some(ord) = order {
                    request = request.order(ord);
                }
                if let Some(c) = is_carousel {
                    request = request.is_carousel(c);
                }

                let tags = request.send().await?;
                println!("{}", serde_json::to_string_pretty(&tags)?);
            }
            Self::Get { id } => {
                let tag = gamma.tags().get(&id).send().await?;
                println!("{}", serde_json::to_string_pretty(&tag)?);
            }
            Self::GetBySlug { slug } => {
                let tag = gamma.tags().get_by_slug(&slug).send().await?;
                println!("{}", serde_json::to_string_pretty(&tag)?);
            }
            Self::Related { id } => {
                // Relationship rows, not tags — see `RelatedTag`.
                let relations = gamma.tags().get_related(&id).send().await?;
                println!("{}", serde_json::to_string_pretty(&relations)?);
            }
            Self::RelatedBySlug { slug } => {
                let relations = gamma.tags().get_related_by_slug(&slug).send().await?;
                println!("{}", serde_json::to_string_pretty(&relations)?);
            }
        }
        Ok(())
    }
}
