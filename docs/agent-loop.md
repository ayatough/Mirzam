# The agent loop

How Mirzam is developed when the author has an hour a week: agents choose the
work, do it and open pull requests; the author reads them, comments and
merges. This page is the arrangement as a whole - what starts an agent, what
it may do, and what the author has to set up once. The agents' own
instructions are [`.github/agent/respond.md`](../.github/agent/respond.md) and
[`.github/agent/build.md`](../.github/agent/build.md), and the standard every
change is held to is [AGENTS.md](../AGENTS.md).

## What the author does

| To | Do this |
|---|---|
| Review a change | Read the pull request. Its description carries the before and after; nothing needs checking out |
| Ask for something different | Comment on the pull request, or on a line of it. An agent starts on it about five minutes later |
| Get an answer now | Put 🚀 anywhere in the comment |
| Take back a comment | Edit or delete it inside the five minutes. The agent reads the thread as it stands when the wait ends, not the comment as first posted |
| Accept a change | Merge it. If that leaves another pull request in conflict, an agent fixes it up unasked |
| Turn a change down | Close the pull request, with a sentence saying why. The agent reads closed pull requests before choosing work and does not offer the same thing again |
| Report a bug | Open an issue: the Markdown that does it, and a screenshot if it is visual. The agent reproduces it and opens a pull request, or asks for what is missing |
| Ask for a feature | Open an issue. Anything with a design decision in it comes back as a proposal first, not as code |
| Approve a proposal, or queue work | Label the issue `agent:ready` |
| Start a run by hand | Actions → Agent → Run workflow, optionally with an issue number |

Nothing else is required. An agent never merges, never pushes to `main`, and
never publishes a release on its own.

## What starts an agent

[`.github/workflows/agent.yml`](../.github/workflows/agent.yml) is all of it.

**The author writes something** (`respond`). An issue opened or edited, an
`agent:ready` label, a comment, a review. The workflow waits five minutes -
every further event on the same thread restarts the wait, so a burst of review
comments is answered once - and then starts an agent on that thread. 🚀 in the
text skips the wait. The flag is in the text, not a reaction, because GitHub
sends no event when somebody reacts to a comment: a reaction could only be
noticed by polling, which is the opposite of "now".

A review, or a comment on a line of the diff, takes one step more than a plain
comment. GitHub starts those workflows from the pull request's branch, and the
Claude app will not give a token to a workflow file that differs from
`main`'s - which any branch cut before the last change to the workflow does.
So for those two events the workflow only asks for itself to be run again from
`main`, naming the thread; the wait and the agent happen in that second run.
A branch older than this arrangement cannot do even that: until `main` has
been merged into it, write in the conversation box rather than as a review.

Only the repository owner's events start anything. The repository is public,
and an agent holds a write token and runs on the owner's Claude subscription;
a stranger's comment is read as material when the owner points at it, and
never obeyed.

Every open pull request from a branch of this repository is answered,
whoever opened it; one from a fork never is, because answering means building
its code on a machine that holds the tokens. The `agent/` prefix only marks
the pull requests the loop opened itself, which is what the daily limit
counts. A Claude Code session that opened a pull request should therefore not
also subscribe to it: two agents answering one comment is worse than none.

**The author merges something** (`mend`). After every merge into `main` the
workflow asks GitHub which open pull requests no longer merge, and starts an
agent on each of those to bring `main` in and resolve the conflict. The loop's
own pull requests nearly all add to the same section of the changelog, so
without this, merging one would leave the next needing a comment before it
could be reviewed. A merge that leaves nothing in conflict starts no agent.

**The clock** (`build`). Once a day the workflow starts an agent with no
thread at all. It takes, in order: an issue labelled `agent:ready`; a bug it
goes looking for and finds; the next specified stage of a workstream; something
that makes the project easier for a stranger to pick up. One pull request per
run.

Before starting, the workflow counts the open `agent/` pull requests, and
**does nothing when three are already waiting**. The author's reading time is
the scarce thing here, and a queue of unread pull requests is not progress.
The limit is the repository variable `AGENT_MAX_OPEN_PRS`.

## What a pull request carries

Its description is the review, so it shows rather than tells:
[`scripts/pr-evidence.sh`](../scripts/pr-evidence.sh) renders the same deck
with the renderer before and after the change and sets the slides side by
side, with what `mirzam check` said about each. AGENTS.md,
[Show it](../AGENTS.md#show-it-the-description-is-the-review), says what the
evidence is for each kind of change.

The pictures live on the `pr-assets` branch, because a description can only
link to a picture that is a file somewhere.
[`.github/workflows/pr-assets.yml`](../.github/workflows/pr-assets.yml)
removes them when the pull request closes.

## Setting it up, once

These are the author's to do; an agent cannot.

1. **Install the Claude GitHub App on this repository**:
   <https://github.com/apps/claude>. The agent's commits, comments and pull
   requests come from `claude[bot]`. That matters twice: a push made with the
   workflow's own token does not start CI, and a pull request opened under the
   author's own account cannot be approved by the author.
2. **Give the workflow a Claude token.** On a machine with Claude Code, run
   `claude setup-token`, and save what it prints as the repository secret
   `CLAUDE_CODE_OAUTH_TOKEN` (Settings → Secrets and variables → Actions). The
   runs then draw on that Claude subscription rather than on API billing. The
   token lasts a year. Until the secret exists, every run of the workflow says
   so and stops, having cost nothing.
3. **Optionally, protect `main`**: require a pull request and green CI. Two
   things to know first. `demo.yml` commits the re-recorded README GIF straight
   to `main` after every merge, so the rule needs `github-actions[bot]` on its
   bypass list. And the rule is the only hard stop there is: the Claude app's
   token can write anywhere in the repository, workflows included, so "never
   push to `main`" and "never touch `.github/workflows/`" are instructions to
   the agent until a branch rule makes the first of them a fact.

## What an agent can reach

A run happens on a GitHub runner that holds this public repository and two
credentials: the Claude token, and a GitHub token that can write to this
repository for as long as the run lasts. Nothing of the author's own machine
is involved.

The agent has a shell and the network, because building and photographing a
deck need both. So what keeps a credential from leaving is, in order: only the
owner can start a run; a fork's code is never built; and the agent reads as
little as possible that the owner did not write - it does not open issues
from other people unless the owner has labelled them, and it does not fetch
documents from the web to test with. Those narrow the ways in; they are not a
sandbox. If the Claude token is ever exposed, delete the secret and issue a
new one with `claude setup-token`.

Restricting the Claude app to this repository alone (its settings, "Only
select repositories") keeps a run's GitHub token away from everything else
the account owns.

## What it costs, and how to stop it

Each agent run spends the subscription's usage; the five-minute wait and the
room check spend none, since no model is called until they pass. A run is
bounded by a turn limit and a timeout in the workflow. The runner minutes are
GitHub's.

To pause the loop, disable the **Agent** workflow in the Actions tab; to stop
it for good, delete the secret. To slow it down, lower `AGENT_MAX_OPEN_PRS` or
change the `cron` line.

## What it does not do yet

- **Fix CI on its own after the run that pushed has ended.** An agent waits
  for CI before finishing and fixes what it finds, but a failure that appears
  later - a new compiler release, a flaky runner - waits for a comment.
- **Answer anybody but the owner.** An issue from a stranger sits until the
  owner comments on it or labels it.
- **Remember across runs.** Each run starts from the repository and the
  threads. What should not be tried twice has to be written where the next run
  will read it: a closing comment on the pull request, or a line in AGENTS.md.
