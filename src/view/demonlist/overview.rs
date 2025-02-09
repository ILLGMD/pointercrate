use crate::{
    config,
    model::{nationality::Nationality, user::User},
    permissions::Permissions,
    state::PointercrateState,
    video,
    view::Page,
    Result, ViewResult,
};
use actix_web::{web::Query, HttpMessage, HttpRequest, HttpResponse};
use actix_web_codegen::get;
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, TimeZone, Utc};
use maud::{html, Markup, PreEscaped};
use serde::Deserialize;
use sqlx::PgConnection;

#[derive(Debug)]
pub struct OverviewDemon {
    pub id: i32,
    pub position: i16,
    pub name: String,
    pub publisher: String,
    pub video: Option<String>,
    pub current_position: Option<i16>,
}

#[derive(Debug)]
pub struct DemonlistOverview {
    pub demon_overview: Vec<OverviewDemon>,
    pub admins: Vec<User>,
    pub mods: Vec<User>,
    pub helpers: Vec<User>,
    pub nations: Vec<Nationality>,

    pub when: Option<DateTime<FixedOffset>>,
    pub query_data: OverviewQueryData,
}

pub async fn overview_demons(connection: &mut PgConnection, at: Option<DateTime<FixedOffset>>) -> Result<Vec<OverviewDemon>> {
    match at {
        None => Ok(sqlx::query_as!(
                OverviewDemon,
                r#"SELECT demons.id, position, demons.name as "name: String", CASE WHEN verifiers.link_banned THEN NULL ELSE video::TEXT END, 
                 players.name as "publisher: String", null::smallint as current_position FROM demons INNER JOIN players ON demons.publisher = players.id INNER JOIN players AS verifiers 
                 ON demons.verifier = verifiers.id WHERE position IS NOT NULL ORDER BY position"#
            )
            .fetch_all(connection)
            .await?),
        Some(time) => Ok(sqlx::query_as!(
                OverviewDemon,
                r#"SELECT demons.id as "id!", position_ as "position!", demons.name as "name!: String", CASE WHEN verifiers.link_banned THEN NULL ELSE video::TEXT END, 
                 players.name as "publisher: String", current_position FROM list_at($1) AS demons INNER JOIN players ON demons.publisher = players.id INNER JOIN players AS verifiers 
                 ON demons.verifier = verifiers.id ORDER BY position_"#, time.naive_utc()
            )
            .fetch_all(connection)
            .await?)

    }
}

impl DemonlistOverview {
    pub(super) fn team_panel(&self) -> Markup {
        let maybe_link = |user: &User| -> Markup {
            html! {
                li {
                    @match user.youtube_channel {
                        Some(ref channel) => a target = "_blank" href = (channel) {
                            (user.name())
                        },
                        None => (user.name())
                    }
                }
            }
        };

        html! {
            section.panel.fade.js-scroll-anim#editors data-anim = "fade" {
                div.underlined {
                    h2 {
                        "List Editors:"
                    }
                }
                p {
                    "Contact any of these people if you have problems with the list or want to see a specific thing changed."
                }
                ul style = "line-height: 30px" {
                    @for admin in &self.admins {
                        b {
                            (maybe_link(admin))
                        }
                    }
                    @for moderator in &self.mods {
                        (maybe_link(moderator))
                    }
                }
                div.underlined {
                    h2 {
                        "List Helpers"
                    }
                }
                p {
                    "Contact these people if you have any questions regarding why a specific record was rejected. Do not needlessly bug them about checking submissions though!"
                }
                ul style = "line-height: 30px" {
                    @for helper in &self.helpers {
                        (maybe_link(helper))
                    }
                }
            }
        }
    }

    pub(super) async fn load(
        connection: &mut PgConnection, when: Option<DateTime<FixedOffset>>, query_data: OverviewQueryData,
    ) -> Result<DemonlistOverview> {
        let admins = User::by_permission(Permissions::ListAdministrator, connection).await?;
        let mods = User::by_permission(Permissions::ListModerator, connection).await?;
        let helpers = User::by_permission(Permissions::ListHelper, connection).await?;

        let nations = Nationality::all(connection).await?;
        let demon_overview = overview_demons(connection, when).await?;

        Ok(DemonlistOverview {
            admins,
            mods,
            helpers,
            nations,
            demon_overview,
            when,
            query_data,
        })
    }
}

#[derive(Deserialize, Debug, Default)]
pub struct OverviewQueryData {
    #[serde(rename = "timemachine", default)]
    time_machine_shown: bool,

    #[serde(rename = "statsviewer", default)]
    stats_viewer_shown: bool,

    #[serde(rename = "submitter", default)]
    record_submitter_shown: bool,
}

#[get("/demonlist/")]
pub async fn index(request: HttpRequest, state: PointercrateState, query_data: Query<OverviewQueryData>) -> ViewResult<HttpResponse> {
    /* static */
    let EARLIEST_DATE: DateTime<FixedOffset> = FixedOffset::east(0).from_utc_datetime(&NaiveDate::from_ymd(2017, 1, 4).and_hms(0, 0, 0));

    let mut connection = state.connection().await?;

    let specified_when = request
        .cookie("when")
        .map(|cookie| DateTime::<FixedOffset>::parse_from_rfc3339(cookie.value()));

    let when = if let Some(when) = specified_when {
        match when {
            Ok(when) if when < EARLIEST_DATE => Some(EARLIEST_DATE),
            Ok(when) if when >= Utc::now() => None,
            Ok(when) => Some(when),
            _ => None,
        }
    } else {
        None
    };

    Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(
        DemonlistOverview::load(&mut connection, when, query_data.into_inner())
            .await?
            .render()
            .0,
    ))
}

impl Page for DemonlistOverview {
    fn title(&self) -> String {
        "Geometry Dash Demonlist".to_string()
    }

    fn description(&self) -> String {
        "The official pointercrate Demonlist!".to_string()
    }

    fn scripts(&self) -> Vec<&str> {
        vec!["js/modules/form.mjs", "js/modules/demonlist.mjs", "js/demonlist.v2.2.js"]
    }

    fn stylesheets(&self) -> Vec<&str> {
        vec!["css/demonlist.v2.1.css", "css/sidebar.css"]
    }

    fn body(&self) -> Markup {
        let dropdowns = super::dropdowns(&self.demon_overview, None);

        html! {
            (super::besides_sidebar_ad())
            (dropdowns)

            div.flex.m-center.container {
                main.left {
                    (time_machine(self.query_data.time_machine_shown))
                    (super::submission_panel(&self.demon_overview, self.query_data.record_submitter_shown))
                    (super::stats_viewer(&self.nations, self.query_data.stats_viewer_shown))
                    @if let Some(when) = self.when {
                        div.panel.fade.blue.flex style="align-items: center;" {
                             span style = "text-align: end"{
                                "You are currently looking at the demonlist how it was on"
                                 br;
                                 b {
                                     @match when.day() {
                                        1 | 21 | 31 => (when.format("%A, %B %est %Y at %l:%M:%S%P GMT%Z")),
                                        2 | 22 => (when.format("%A, %B %end %Y at %l:%M:%S%P GMT%Z")),
                                        _ => (when.format("%A, %B %eth %Y at %l:%M:%S%P GMT%Z"))
                                     }
                                 }
                             }
                             a.white.button href = "/demonlist/" onclick=r#"document.cookie = "when=""# style = "margin-left: 15px"{ b{"Go to present" }}
                        }
                    }
                    @for demon in &self.demon_overview {
                        @if demon.position <= config::extended_list_size() {
                            section.panel.fade style="overflow:hidden" {
                                @if let Some(ref video) = demon.video {
                                    div.flex style = "align-items: center" {
                                        div.thumb."ratio-16-9"."js-delay-css" style = "position: relative" data-property = "background-image" data-property-value = {"url('" (video::thumbnail(video)) "')"} {
                                            a.play href = (video) {}
                                        }
                                        div style = "padding-left: 15px" {
                                            h2 style = "text-align: left; margin-bottom: 0px" {
                                                a href = {"/demonlist/permalink/" (demon.id) "/"} {
                                                    "#" (demon.position) (PreEscaped(" &#8211; ")) (demon.name)
                                                }
                                            }
                                            h3 style = "text-align: left" {
                                                i {
                                                    (demon.publisher)
                                                }
                                                @if let Some(current_position) = demon.current_position {
                                                    br;
                                                    @if current_position > config::extended_list_size() {
                                                        "Currently Legacy"
                                                    }
                                                    @else {
                                                        "Currently #"(current_position)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                @else {
                                    div.flex.col style = "align-items: center" {
                                        h2 style = "margin-bottom: 0px"{
                                            a href = {"/demonlist/permalink/" (demon.id) "/"} {
                                                "#" (demon.position) (PreEscaped(" &#8211; ")) (demon.name)
                                            }
                                        }
                                        h3 {
                                            i {
                                                (demon.publisher)
                                            }
                                            @if let Some(current_position) = demon.current_position {
                                                br;
                                                @if current_position > config::extended_list_size() {
                                                    "Currently Legacy"
                                                }
                                                @else {
                                                    "Currently #"(current_position)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            @if demon.position == 1 {
                                section.panel.fade style = "padding: 0px; height: 90px"{
                                (PreEscaped(format!(r#"
                                    <script async src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js"></script>
                                    <!-- Demonlist Responsive Feed Ad -->
                                    <ins class="adsbygoogle"
                                         style="display:inline-block;width:728px;height:90px"
                                         data-ad-client="{}"
                                         data-ad-slot="2819150519"></ins>
                                    <script>
                                         (adsbygoogle = window.adsbygoogle || []).push({{}});
                                    </script>
                                    "#, config::adsense_publisher_id())))
                                }
                            }
                            // Place ad every 20th demon
                            @if demon.position % 20 == 0 {
                                section.panel.fade {
                                (PreEscaped(format!(r#"
                                    <script async src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js"></script>
                                    <ins class="adsbygoogle"
                                         style="display:block"
                                         data-ad-format="fluid"
                                         data-ad-layout-key="-h1+40+4u-93+n"
                                         data-ad-client="{}"
                                         data-ad-slot="5157884729"></ins>
                                    <script>
                                         (adsbygoogle = window.adsbygoogle || []).push({{}});
                                    </script>
                                    "#, config::adsense_publisher_id())))
                                }
                            }
                        }
                    }
                }

                aside.right {
                    (self.team_panel())
                    (super::sidebar_ad())
                    (super::rules_panel())
                    (super::submit_panel())
                    (super::stats_viewer_panel())
                    (super::discord_panel())
                }
            }

        }
    }

    fn head(&self) -> Vec<Markup> {
        vec![
            html! {
            (PreEscaped(r#"
                <link href="https://cdnjs.cloudflare.com/ajax/libs/flag-icon-css/3.4.3/css/flag-icon.min.css" rel="stylesheet">
                <script type="application/ld+json">
                {
                    "@context": "http://schema.org",
                    "@type": "WebPage",
                    "breadcrumb": {
                        "@type": "BreadcrumbList",
                        "itemListElement": [
                            {
                                "@type": "ListItem",
                                "position": 1,
                                "item": {
                                    "@id": "https://pointercrate.com/",
                                    "name": "pointercrate"
                                }
                            },
                            {
                                "@type": "ListItem",
                                "position": 2,
                                "item": {
                                    "@id": "https://pointercrate.com/demonlist/",
                                    "name": "demonlist"
                                }
                            }
                        ]
                    },
                    "name": "Geometry Dash Demonlist",
                    "description": "The official pointercrate Demonlist!",
                    "url": "https://pointercrate.com/demonlist/"
                }
                </script>
            "#))
            },
            html! {
                (PreEscaped(format!("
                    <script>
                        window.list_length = {0};
                        window.extended_list_length = {1}
                    </script>", config::list_size(), config::extended_list_size())
                ))
            },
            html! {
                link ref = "canonical" href = "https://pointercrate.com/demonlist/";
            },
        ]
    }
}

fn time_machine(visible: bool) -> Markup {
    let current_year = FixedOffset::east(3600 * 23 + 3599)
        .from_utc_datetime(&Utc::now().naive_utc())
        .year();

    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];

    html! {
        section.panel.fade.closable#time-machine  style=(if !visible {"display:none;overflow: initial"} else {"overflow: initial"}) {
            span.plus.cross.hover {}
            form#time-machine-form novalidate = "" {
                div.underlined {
                    h2 {"Time Machine"}
                }
                p {
                    "Enter the date you want to view the demonlist at below. For technical reasons, the earliest possible date is January 4th 2017. Note however that data before August 4th 2017 is only provided on a best-effort basis and not guaranteed to be 100% accurate. Particularly data from before April 4th 2017 contains significant errors!"
                }
                div.flex {
                    span.form-input data-type = "dropdown" style = "max-width:33%" {
                        h3 {"Year:"}
                        (crate::view::simple_dropdown("time-machine-year", None, 2017..=current_year))
                        p.error {}
                    }
                    span.form-input data-type = "dropdown" style = "max-width:33%"  {
                        h3 {"Month:"}
                        (crate::view::simple_dropdown("time-machine-month", None, months.iter()))
                        p.error {}
                    }
                    span.form-input data-type = "dropdown" style = "max-width:33%"  {
                        h3 {"Day:"}
                        (crate::view::simple_dropdown("time-machine-day", None, 1..=31))
                        p.error {}
                    }
                }
                div.flex {
                    span.form-input data-type = "dropdown" style = "max-width:33%" {
                        h3 {"Hour:"}
                        (crate::view::simple_dropdown("time-machine-hour", Some(0), 0..24))
                        p.error {}
                    }
                    span.form-input data-type = "dropdown" style = "max-width:33%"  {
                        h3 {"Minute:"}
                        (crate::view::simple_dropdown("time-machine-minute", Some(0), 0..=59))
                        p.error {}
                    }
                    span.form-input data-type = "dropdown" style = "max-width:33%"  {
                        h3 {"Second:"}
                        (crate::view::simple_dropdown("time-machine-second", Some(0), 0..=59))
                        p.error {}
                    }
                }
                input.button.blue.hover type = "submit" style = "margin: 15px auto 0px;" value="Let's goooo!";
            }
        }
    }
}
