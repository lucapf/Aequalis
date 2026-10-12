import { Action, ActionPanel, Clipboard, Detail, getPreferenceValues, Icon, List, showHUD } from "@raycast/api";
import { useCachedState } from "@raycast/utils";
import { useEffect, useRef, useState } from "react";
import { Entry, headline, locateTally, remember, Reply, run, valueOf, words } from "./tally";

interface Preferences {
  tallyPath?: string;
  offline?: boolean;
}

/// Calculate with Tally: type a request, the answer appears as you type; Enter copies it. An
/// ambiguous request lists its readings: choosing one writes it into the search bar.
export default function Calculate() {
  const { tallyPath, offline = false } = getPreferenceValues<Preferences>();
  const path = locateTally(tallyPath);
  const [query, setQuery] = useState("");
  const [reply, setReply] = useState<Reply | undefined>();
  const [loading, setLoading] = useState(false);
  const [recent, setRecent] = useCachedState<Entry[]>("recent", []);
  // Only the newest request's reply is shown.
  const latest = useRef(0);

  useEffect(() => {
    const request = words(query);
    if (!path || request.length === 0) {
      setReply(undefined);
      return;
    }
    const id = ++latest.current;
    setLoading(true);
    run(path, request, offline).then((r) => {
      if (id !== latest.current) return;
      setReply(r);
      setLoading(false);
    });
  }, [query, path, offline]);

  async function copy(r: Reply) {
    if (!r.ok || !r.result) return;
    const value = valueOf(r.result);
    setRecent(remember({ input: query.trim(), result: r.result }, recent));
    await Clipboard.copy(value);
    await showHUD(`Copied ${value}`);
  }

  const about = path ? (
    <Action.Push icon={Icon.Info} title="About Tally" target={<About path={path} offline={offline} />} />
  ) : null;

  if (!path) {
    return (
      <List>
        <List.EmptyView
          icon={Icon.ExclamationMark}
          title="The tally command was not found"
          description="Install it in ~/.local/bin, /opt/homebrew/bin or /usr/local/bin, or set its path in the extension's preferences."
          actions={
            <ActionPanel>
              <Action.OpenInBrowser title="Download Tally" url="https://github.com/lucapf/tally/releases" />
            </ActionPanel>
          }
        />
      </List>
    );
  }

  return (
    <List
      searchText={query}
      onSearchTextChange={setQuery}
      filtering={false}
      throttle
      isLoading={loading}
      searchBarPlaceholder="10 km mi · 12+4/2 · 100 usd eur · now Tokyo"
    >
      {reply && (
        <List.Section title="Answer">
          <List.Item
            icon={reply.ok ? Icon.CheckCircle : Icon.QuestionMark}
            title={headline(reply)}
            subtitle={reply.ok ? [...(reply.warnings ?? []), ...(reply.notes ?? [])].join(" · ") : undefined}
            actions={
              <ActionPanel>
                {reply.ok && reply.result && (
                  <>
                    <Action title="Copy Answer" icon={Icon.Clipboard} onAction={() => copy(reply)} />
                    <Action.Paste title="Paste Answer" content={valueOf(reply.result)} />
                    <Action.CopyToClipboard
                      title="Copy Request and Answer"
                      content={`${query.trim()} = ${reply.result}`}
                    />
                  </>
                )}
                {about}
              </ActionPanel>
            }
          />
          {reply.ambiguous?.choices.map((choice) => (
            <List.Item
              key={choice.request}
              icon={Icon.ArrowRight}
              title={[choice.pattern, choice.date].filter(Boolean).join("   ")}
              subtitle={choice.request}
              actions={
                <ActionPanel>
                  <Action title="Use This Reading" icon={Icon.ArrowRight} onAction={() => setQuery(choice.request)} />
                </ActionPanel>
              }
            />
          ))}
        </List.Section>
      )}
      {recent.length > 0 && (
        <List.Section title="Recent">
          {recent.map((entry) => (
            <List.Item
              key={entry.input}
              icon={Icon.Clock}
              title={entry.input}
              subtitle={`= ${entry.result}`}
              actions={
                <ActionPanel>
                  <Action title="Use This Request" icon={Icon.ArrowRight} onAction={() => setQuery(entry.input)} />
                  <Action.CopyToClipboard title="Copy Answer" content={valueOf(entry.result)} />
                  <Action
                    title="Clear Recent"
                    icon={Icon.Trash}
                    style={Action.Style.Destructive}
                    onAction={() => setRecent([])}
                  />
                  {about}
                </ActionPanel>
              }
            />
          ))}
        </List.Section>
      )}
      {!reply && recent.length === 0 && (
        <List.EmptyView
          icon={Icon.Calculator}
          title="Type a request"
          description="Units (10 km mi), the calculator (12+4/2), currencies (100 usd eur), dates (2026/10/05 + 5wd), time zones (10:00 Paris to Tokyo)"
          actions={<ActionPanel>{about}</ActionPanel>}
        />
      )}
    </List>
  );
}

/// About: `tally --about` (the logo, the version, the memory, the last online sync).
function About({ path, offline }: { path: string; offline: boolean }) {
  const [text, setText] = useState<string | undefined>();
  useEffect(() => {
    run(path, ["--about"], offline).then((r) => setText(r.result ?? r.error ?? ""));
  }, [path, offline]);
  return <Detail isLoading={text === undefined} markdown={text === undefined ? "" : "```\n" + text + "\n```"} />;
}
