// Downloads a Tricount group and writes it as a CSV file that ezcount imports
// ("Import a CSV file" on the home screen).
//
//   bun scripts/tricount-to-csv.ts <share link or key> [out.csv]
//
// The share link is in Tricount under the group's Share button (https://tricount.com/t…).
// Tricount has no export and no public API: this uses the one its phone app talks to, as an
// anonymous device, so it may stop working when Tricount changes it. Income entries have no
// equivalent in ezcount and are skipped.
import { generateKeyPairSync, randomUUID } from "node:crypto";
import { writeFileSync } from "node:fs";

interface Money {
  currency: string;
  value: string;
}
interface Membership {
  RegistryMembershipNonUser: { alias: { display_name: string } };
}
interface Allocation {
  amount: Money;
  amount_local: Money;
  membership: Membership;
  // A set amount, or a number of parts (`share_ratio`) of what the set amounts leave.
  type: "AMOUNT" | "RATIO";
  share_ratio?: number;
}
interface Entry {
  // In the group's currency, and as paid.
  amount: Money;
  amount_local: Money;
  description: string;
  type_transaction: string;
  membership_owned: Membership;
  allocations: Allocation[];
  date: string;
}
interface Registry {
  title: string;
  currency: string;
  memberships: Membership[];
  all_registry_entry: { RegistryEntry: Entry }[];
}

const [input, outArg] = process.argv.slice(2);
if (!input) {
  console.error("usage: bun scripts/tricount-to-csv.ts <share link or key> [out.csv]");
  process.exit(1);
}
// https://tricount.com/tAbCdEf -> tAbCdEf
const key =
  input
    .trim()
    .replace(/[/?#]+$/, "")
    .split("/")
    .pop() ?? "";

const base = "https://api.tricount.bunq.com";
const appId = randomUUID();
const headers: Record<string, string> = {
  "User-Agent": "com.bunq.tricount.android:RELEASE:7.0.7:3174:ANDROID:13:C",
  "app-id": appId,
  "X-Bunq-Client-Request-Id": randomUUID(),
  "Content-Type": "application/json",
};

async function fetchRegistry(): Promise<Registry> {
  const { publicKey } = generateKeyPairSync("rsa", {
    modulusLength: 2048,
    publicKeyEncoding: { type: "spki", format: "pem" },
    privateKeyEncoding: { type: "pkcs8", format: "pem" },
  });
  const session = await fetch(`${base}/v1/session-registry-installation`, {
    method: "POST",
    headers,
    body: JSON.stringify({
      app_installation_uuid: appId,
      client_public_key: publicKey,
      device_description: "Android",
    }),
  });
  if (!session.ok) throw new Error(`Tricount refused the session: ${session.status}`);
  const items: { Token?: { token: string }; UserPerson?: { id: number } }[] = (await session.json())
    .Response;
  const token = items.find((i) => i.Token)?.Token?.token;
  const userId = items.find((i) => i.UserPerson)?.UserPerson?.id;
  if (!token || !userId) throw new Error("Tricount's session has no token or user");

  const res = await fetch(
    `${base}/v1/user/${userId}/registry?public_identifier_token=${encodeURIComponent(key)}`,
    { headers: { ...headers, "X-Bunq-Client-Authentication": token } }
  );
  if (!res.ok) throw new Error(`Tricount has no group for this link: ${res.status}`);
  const registry = (await res.json()).Response?.[0]?.Registry;
  if (!registry) throw new Error("Tricount's answer has no group");
  return registry;
}

const nameOf = (m: Membership) => m.RegistryMembershipNonUser.alias.display_name;
// Amounts are negative strings with two decimals ("-134.00").
const positive = (money: Money) => money.value.replace(/^-/, "");
const cell = (text: string) => (/[",\n\r]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text);

const registry = await fetchRegistry();
const names = registry.memberships.map(nameOf);
const lines = [
  [
    "Date",
    "Title",
    "Amount",
    "Currency",
    "Paid by",
    "Type",
    "Original amount",
    "Original currency",
    "Exchange rate",
    "Split",
    ...names,
  ],
];
let skipped = 0;
for (const { RegistryEntry: entry } of registry.all_registry_entry) {
  const payment = entry.type_transaction === "BALANCE";
  if (!entry.amount.value.startsWith("-") || (!payment && entry.type_transaction !== "NORMAL")) {
    skipped += 1;
    console.warn(`Skipped (${entry.type_transaction}): ${entry.date} ${entry.description}`);
    continue;
  }
  const allocations = new Map(entry.allocations.map((a) => [nameOf(a.membership), a]));
  const foreign = entry.amount_local.currency !== registry.currency;
  // Per person: parts, a set amount (in the currency paid), or "-" for someone who isn't in.
  const split = names.map((name) => {
    const a = allocations.get(name);
    if (!a) return "-";
    if (a.type === "RATIO") return String(a.share_ratio ?? 1);
    return Number(a.amount_local.value) === 0 ? "-" : positive(a.amount_local);
  });
  lines.push([
    entry.date,
    // ezcount titles payments itself, as "Payment: A → B".
    payment ? "" : entry.description,
    positive(entry.amount),
    registry.currency,
    nameOf(entry.membership_owned),
    payment ? "payment" : "expense",
    foreign ? positive(entry.amount_local) : "",
    foreign ? entry.amount_local.currency : "",
    // ezcount works the rate out from the two amounts.
    "",
    payment ? "" : split.join(" "),
    ...names.map((name) => {
      const a = allocations.get(name);
      return a ? positive(a.amount) : "";
    }),
  ]);
}

const out = outArg ?? `${registry.title.replace(/[\\/:*?"<>|]/g, "_")}.csv`;
writeFileSync(out, `${lines.map((line) => line.map(cell).join(",")).join("\n")}\n`);
console.log(
  `${out}: "${registry.title}", ${names.length} people, ${lines.length - 1} lines` +
    (skipped ? `, ${skipped} skipped` : "")
);
