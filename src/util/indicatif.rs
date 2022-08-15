use console::{strip_ansi_codes, Style};
use indicatif::{MultiProgress, ProgressBar, ProgressFinish, ProgressStyle};

use std::borrow::Cow;
use std::time::Duration;

pub trait IndicatifHelpers {
    fn with_steady_tick(self, duration: Duration) -> Self;

    fn restyle_message(&self, style: Style);

    fn message_unstyled(&self) -> String;
}

impl IndicatifHelpers for ProgressBar {
    fn with_steady_tick(self, duration: Duration) -> Self {
        self.enable_steady_tick(duration);
        self
    }

    fn message_unstyled(&self) -> String {
        strip_ansi_codes(&self.message()).to_string()
    }

    fn restyle_message(&self, style: Style) {
        let msg = self.message_unstyled();
        self.set_message(style.apply_to(msg).to_string());
    }
}

pub trait MultiProgressHelpers {
    fn println_after(
        &self,
        after: &ProgressBar,
        message: impl Into<Cow<'static, str>>,
    ) -> ProgressBar;

    fn println_before(
        &self,
        after: &ProgressBar,
        message: impl Into<Cow<'static, str>>,
    ) -> ProgressBar;

    fn note(&self, s: &str);
    fn warn(&self, s: &str);
    fn error(&self, s: &str);
}

impl MultiProgressHelpers for MultiProgress {
    fn println_after(
        &self,
        after: &ProgressBar,
        message: impl Into<Cow<'static, str>>,
    ) -> ProgressBar {
        let ret = self.insert_after(
            after,
            ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("{msg}").unwrap())
                .with_finish(ProgressFinish::AndLeave)
                .with_message(message)
                .with_tab_width(4),
        );
        ret.tick();
        ret
    }

    fn println_before(
        &self,
        after: &ProgressBar,
        message: impl Into<Cow<'static, str>>,
    ) -> ProgressBar {
        let ret = self.insert_before(
            after,
            ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("{msg}").unwrap())
                .with_finish(ProgressFinish::AndLeave)
                .with_message(message)
                .with_tab_width(4),
        );
        ret.tick();
        ret
    }

    fn note(&self, s: &str) {
        let header = Style::from_dotted_str("cyan.bold").apply_to("note");
        self.suspend(|| eprintln!("{}: {}", header, s));
    }

    fn warn(&self, s: &str) {
        let header = Style::from_dotted_str("yellow.bold").apply_to("warning");
        self.suspend(|| eprintln!("{}: {}", header, s));
    }

    fn error(&self, s: &str) {
        let header = Style::from_dotted_str("red.bold").apply_to("error");
        self.suspend(|| eprintln!("\n{}: {}\n", header, s));
    }
}
