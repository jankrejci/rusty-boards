import re

from gitlint.rules import CommitMessageBody, CommitRule, LineRule, RuleViolation
from gitlint.options import ListOption

EMOJI_PATTERN = re.compile(
    "[\U0001f300-\U0001f9ff\u2600-\u26ff\u2700-\u27bf]"
)


class BodyBulletFormat(LineRule):
    """Body lines must be blank, start with '- ', or be indented continuation."""

    name = "body-bullet-format"
    id = "UC1"
    target = CommitMessageBody

    def validate(self, line, _commit):
        if not line:
            return
        if line.startswith("- ") or line.startswith("  "):
            return
        return [RuleViolation(
            self.id,
            f"body line must be blank, bullet '- ', or indented continuation: {line}",
        )]


class NoBannedContent(CommitRule):
    """Reject Co-Authored-By and emojis in commit messages."""

    name = "no-banned-content"
    id = "UC2"
    options_spec = [
        ListOption("banned", ["Co-Authored-By"], "Banned phrases in body"),
    ]

    def validate(self, commit):
        violations = []

        for phrase in self.options["banned"].value:
            for i, line in enumerate(commit.message.body, start=2):
                if phrase.lower() in line.lower():
                    violations.append(RuleViolation(
                        self.id, f"body must not contain '{phrase}'",
                        line, line_nr=i,
                    ))

        full = commit.message.original
        if EMOJI_PATTERN.search(full):
            violations.append(RuleViolation(
                self.id, "commit message must not contain emojis",
            ))

        return violations or None
