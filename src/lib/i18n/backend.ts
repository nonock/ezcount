// Text that comes from the Rust core in English (errors, sync failures, password advice, edit
// summaries), translated here by pattern. What isn't listed shows in English.

import { categoryName } from "../categories";
import { i18n, type Language } from "./index.svelte";

type Pattern = [RegExp, string | ((...groups: string[]) => string)];

/** A category's key ("food", or "none") as edit summaries write it, named. */
const category = (key: string) => categoryName(key === "none" ? null : key);

const FR: Pattern[] = [
  // Groups, members and expenses
  [/^Group name cannot be empty$/, "Le nom du groupe ne peut pas être vide"],
  [/^Participant name cannot be empty$/, "Le nom du participant ne peut pas être vide"],
  [/^Group not found$/, "Groupe introuvable"],
  [/^Expense not found$/, "Dépense introuvable"],
  [/^Participant not found$/, "Participant introuvable"],
  [/^Amount must be greater than zero$/, "Le montant doit être supérieur à zéro"],
  [/^Amount is too large$/, "Le montant est trop grand"],
  [
    /^Reimbursement amount must be greater than zero$/,
    "Le montant du remboursement doit être supérieur à zéro",
  ],
  [
    /^Sender and recipient cannot be the same person$/,
    "La même personne ne peut pas payer et recevoir",
  ],
  [
    /^Expense must be split among at least one participant$/,
    "La dépense doit être répartie entre au moins une personne",
  ],
  [
    /^A participant appears twice in the split$/,
    "Une personne apparaît deux fois dans la répartition",
  ],
  [/^Shares must be at least 1$/, "Il faut au moins 1 part"],
  [/^This category can't be used$/, "Cette catégorie ne peut pas être utilisée"],
  [/^This expense is no longer in the trash$/, "Cette dépense n'est plus dans la corbeille"],
  [/^This repeated expense no longer exists$/, "Cette dépense récurrente n'existe plus"],
  [
    /^An expense repeats every week, month or year$/,
    "Une dépense se répète chaque semaine, chaque mois ou chaque année",
  ],
  [
    /^A repeated expense has to be in the group's currency$/,
    "Une dépense récurrente doit être dans la devise du groupe",
  ],
  [
    /^The payers of this expense are not valid$/,
    "Les payeurs de cette dépense ne sont pas valides",
  ],
  [
    /^A participant appears twice among the payers$/,
    "Une personne apparaît deux fois parmi les payeurs",
  ],
  [
    /^What each payer paid must be above zero$/,
    "Ce que chaque payeur a payé doit être supérieur à zéro",
  ],
  [
    /^The payers paid (.*) between them, not the expense's (.*)$/,
    (paid, amount) => `Les payeurs ont payé ${paid} à eux tous, et non les ${amount} de la dépense`,
  ],
  [/^A fixed amount must be above zero$/, "Un montant fixe doit être supérieur à zéro"],
  [
    /^The payer is not an active member of this group$/,
    "La personne qui paie n'est pas un membre actif de ce groupe",
  ],
  [
    /^The currency must be a three-letter code, such as EUR$/,
    "La devise doit être un code de trois lettres, comme EUR",
  ],
  [
    /^The exchange rate must be a number above zero, such as 0\.92$/,
    "Le taux de change doit être un nombre supérieur à zéro, comme 0.92",
  ],
  [
    /^The expense's currency or exchange rate is not valid$/,
    "La devise ou le taux de change de la dépense n'est pas valide",
  ],
  [/^This person is not a member of the group$/, "Cette personne n'est pas membre du groupe"],
  [/^Add yourself to the group$/, "Ajoutez-vous au groupe"],
  [/^This group was deleted$/, "Ce groupe a été supprimé"],
  [
    /^Say who you are in this group before asking to delete it$/,
    "Dites qui vous êtes dans ce groupe avant de demander sa suppression",
  ],
  [/^This file name can't be used$/, "Ce nom de fichier ne peut pas être utilisé"],
  [/^Could not find the Downloads folder$/, "Dossier Téléchargements introuvable"],
  [/^Could not save the file: (.*)$/, (why) => `Impossible d'enregistrer le fichier : ${why}`],
  [
    /^This picture can't be used: pick a JPEG, PNG or WebP image$/,
    "Cette image ne peut pas être utilisée : choisissez une image JPEG, PNG ou WebP",
  ],
  [/^This picture is too big$/, "Cette image est trop lourde"],
  [
    /^This name is too long \((\d+) characters at most\)$/,
    (max) => `Ce nom est trop long (${max} caractères au plus)`,
  ],
  [
    /^This description is too long \((\d+) characters at most\)$/,
    (max) => `Cette description est trop longue (${max} caractères au plus)`,
  ],

  // CSV files
  [/^Line (\d+): (.*)$/s, (line, rest) => `Ligne ${line} : ${translate(rest, FR)}`],
  [/^Line (\d+) can't be read: (.*)$/s, (line, e) => `La ligne ${line} est illisible : ${e}`],
  [/^nobody shares this expense$/, "personne ne partage cette dépense"],
  [/^the parts are too uneven to import$/, "les parts sont trop inégales pour être importées"],
  [/^(.*) paid, but has no column$/, (payer) => `${payer} a payé, mais n'a pas de colonne`],
  [/^A person's column has no name$/, "La colonne d'une personne n'a pas de nom"],
  [/^Two columns are named (.*)$/, (name) => `Deux colonnes s'appellent ${name}`],

  // Accounts
  [/^Wrong username or password$/, "Nom d'utilisateur ou mot de passe incorrect"],
  [/^Wrong username or recovery key$/, "Nom d'utilisateur ou clé de récupération incorrect"],
  [/^Wrong password$/, "Mot de passe incorrect"],
  [/^Your current password is wrong$/, "Votre mot de passe actuel est incorrect"],
  [
    /^The username "(.*)" is already taken$/,
    (name) => `Le nom d'utilisateur « ${name} » est déjà pris`,
  ],
  [
    /^Usernames are 3 to 32 letters, digits, dots, dashes or underscores$/,
    "Un nom d'utilisateur fait 3 à 32 lettres, chiffres, points, tirets ou tirets bas",
  ],
  [
    /^Use a password of at least (\d+) characters$/,
    (n) => `Utilisez un mot de passe d'au moins ${n} caractères`,
  ],
  [
    /^This password is too easy to guess\.(?: (.*))? Try a few unrelated words\.$/,
    (why) =>
      `Ce mot de passe est trop facile à deviner.${why ? ` ${translate(why, FR)}` : ""} Essayez quelques mots sans rapport entre eux.`,
  ],
  [
    /^Too many failed attempts\. Wait a few minutes and try again\.$/,
    "Trop de tentatives échouées. Attendez quelques minutes et réessayez.",
  ],
  [
    /^This isn't a valid recovery key\. Check it for typos\.$/,
    "Cette clé de récupération n'est pas valide. Vérifiez qu'elle ne contient pas de faute de frappe.",
  ],
  [/^You are not logged in$/, "Vous n'êtes pas connecté"],
  [/^Log in first$/, "Connectez-vous d'abord"],
  [/^This device is already logged in$/, "Cet appareil est déjà connecté"],
  [/^This device has no key for your account$/, "Cet appareil n'a pas la clé de votre compte"],
  [
    /^Your account key could not be decrypted$/,
    "La clé de votre compte n'a pas pu être déchiffrée",
  ],
  [
    /^Some changes on this device are not uploaded yet\. Connect to the internet and try again, or log out anyway and lose them\.$/,
    "Certains changements de cet appareil ne sont pas encore envoyés. Connectez-vous à Internet et réessayez, ou déconnectez-vous quand même et perdez-les.",
  ],
  [
    /^Your account data on this device was unreadable\. Log in again to restore it\.$/,
    "Les données de votre compte sur cet appareil étaient illisibles. Reconnectez-vous pour les restaurer.",
  ],

  [/^This is not an ezcount login code$/, "Ce n'est pas un code de connexion ezcount"],
  [
    /^This code has expired or was already used\. Show a new one and scan it\.$/,
    "Ce code a expiré ou a déjà été utilisé. Affichez-en un nouveau et scannez-le.",
  ],
  [
    /^This server can't connect devices with a code yet\. Update the ezcount relay\.$/,
    "Ce serveur ne sait pas encore connecter des appareils avec un code. Mettez à jour le relais ezcount.",
  ],
  [
    /^The sync server sent corrupt account data$/,
    "Le serveur de synchronisation a envoyé des données de compte corrompues",
  ],

  // Invites
  [/^This is not a valid ezcount invite$/, "Ce n'est pas une invitation ezcount valide"],
  [
    /^This invite is for a different version of ezcount\. Ask for a new one\.$/,
    "Cette invitation est pour une autre version d'ezcount. Demandez-en une nouvelle.",
  ],
  [/^This group is already in your account$/, "Ce groupe est déjà dans votre compte"],
  [
    /^The invite code does not match the group on the server$/,
    "Le code d'invitation ne correspond pas au groupe sur le serveur",
  ],
  [/^The group key is malformed$/, "La clé du groupe est mal formée"],

  // The sync server
  [/^Could not reach the sync server$/, "Serveur de synchronisation injoignable"],
  [/^Sync request failed: (.*)$/s, (e) => `La synchronisation a échoué : ${e}`],
  [
    /^The server URL must start with https:\/\/$/,
    "L'adresse du serveur doit commencer par https://",
  ],
  [
    /^Use an https:\/\/ address\. Plain http:\/\/ only works for a server on this device or your local network: over the internet it would send your login unencrypted\.$/,
    "Utilisez une adresse https://. Le simple http:// ne marche que pour un serveur sur cet appareil ou votre réseau local : sur Internet, il enverrait vos identifiants en clair.",
  ],
  [
    /^The sync server redirects to (.*), which ezcount doesn't follow\. Check the server address\.$/,
    (target) =>
      `Le serveur de synchronisation redirige vers ${target}, ce qu'ezcount ne suit pas. Vérifiez l'adresse du serveur.`,
  ],
  [
    /^The sync server rejected this group's key$/,
    "Le serveur de synchronisation a refusé la clé de ce groupe",
  ],
  [
    /^The sync server does not know this group$/,
    "Le serveur de synchronisation ne connaît pas ce groupe",
  ],
  [
    /^This group has reached the sync server's size limit$/,
    "Ce groupe a atteint la taille maximale du serveur de synchronisation",
  ],
  [
    /^The sync server is getting too many requests from your network\. Try again in a while\.$/,
    "Le serveur de synchronisation reçoit trop de requêtes depuis votre réseau. Réessayez dans un moment.",
  ],
  [
    /^The sync server is full\. Try again later\.$/,
    "Le serveur de synchronisation est plein. Réessayez plus tard.",
  ],
  [
    /^The sync server answered (\d+): (.*)$/s,
    (status, body) => `Le serveur de synchronisation a répondu ${status} : ${body}`,
  ],
  [
    /^The sync server has no usable data for this group$/,
    "Le serveur de synchronisation n'a pas de données utilisables pour ce groupe",
  ],
  [
    /^This server doesn't support accounts\. Update the ezcount relay\.$/,
    "Ce serveur ne gère pas les comptes. Mettez à jour le relais ezcount.",
  ],
  [
    /^This server can't change passwords or recovery keys yet\. Update the ezcount relay\.$/,
    "Ce serveur ne sait pas encore changer les mots de passe ni les clés de récupération. Mettez à jour le relais ezcount.",
  ],
  [/^This group is not shared$/, "Ce groupe n'est pas partagé"],
  [/^Sharing is not available on this device$/, "Le partage n'est pas disponible sur cet appareil"],

  // Password advice (zxcvbn)
  [/^Straight rows of keys are easy to guess\.$/, "Les rangées de touches sont faciles à deviner."],
  [
    /^Short keyboard patterns are easy to guess\.$/,
    "Les courts motifs de clavier sont faciles à deviner.",
  ],
  [
    /^Repeats like "aaa" are easy to guess\.$/,
    "Les répétitions comme « aaa » sont faciles à deviner.",
  ],
  [
    /^Repeats like "abcabcabc" are only slightly harder to guess than "abc"\.$/,
    "Les répétitions comme « abcabcabc » sont à peine plus dures à deviner que « abc ».",
  ],
  [/^This is a top-10 common password\.$/, "C'est l'un des 10 mots de passe les plus courants."],
  [/^This is a top-100 common password\.$/, "C'est l'un des 100 mots de passe les plus courants."],
  [/^This is a very common password\.$/, "C'est un mot de passe très courant."],
  [/^This is similar to a commonly used password\.$/, "Il ressemble à un mot de passe courant."],
  [
    /^Sequences like abc or 6543 are easy to guess\.$/,
    "Les suites comme abc ou 6543 sont faciles à deviner.",
  ],
  [/^Recent years are easy to guess\.$/, "Les années récentes sont faciles à deviner."],
  [/^A word by itself is easy to guess\.$/, "Un mot seul est facile à deviner."],
  [/^Dates are often easy to guess\.$/, "Les dates sont souvent faciles à deviner."],
  [
    /^Names and surnames by themselves are easy to guess\.$/,
    "Les prénoms et noms seuls sont faciles à deviner.",
  ],
  [
    /^Common names and surnames are easy to guess\.$/,
    "Les prénoms et noms courants sont faciles à deviner.",
  ],
  [
    /^Use a few words, avoid common phrases\.$/,
    "Utilisez quelques mots, évitez les expressions courantes.",
  ],
  [
    /^No need for symbols, digits, or uppercase letters\.$/,
    "Pas besoin de symboles, de chiffres ni de majuscules.",
  ],
  [
    /^Add another word or two\. Uncommon words are better\.$/,
    "Ajoutez un ou deux mots. Les mots rares sont meilleurs.",
  ],
  [/^Capitalization doesn't help very much\.$/, "Les majuscules n'aident pas beaucoup."],
  [
    /^All-uppercase is almost as easy to guess as all-lowercase\.$/,
    "Tout en majuscules est presque aussi facile à deviner que tout en minuscules.",
  ],
  [
    /^Reversed words aren't much harder to guess\.$/,
    "Les mots à l'envers ne sont pas beaucoup plus durs à deviner.",
  ],
  [
    /^Predictable substitutions like '@' instead of 'a' don't help very much\.$/,
    "Les substitutions prévisibles comme « @ » pour « a » n'aident pas beaucoup.",
  ],
  [
    /^Use a longer keyboard pattern with more turns\.$/,
    "Utilisez un motif de clavier plus long, avec plus de changements de direction.",
  ],
  [/^Avoid repeated words and characters\.$/, "Évitez les mots et caractères répétés."],
  [/^Avoid sequences\.$/, "Évitez les suites."],
  [/^Avoid recent years\.$/, "Évitez les années récentes."],
  [/^Avoid years that are associated with you\.$/, "Évitez les années liées à vous."],
  [
    /^Avoid dates and years that are associated with you\.$/,
    "Évitez les dates et années liées à vous.",
  ],

  // Feedback
  [/^Write a message first$/, "Écrivez d'abord un message"],
  [
    /^This message is too long ((d+) characters at most)$/,
    (n) => `Ce message est trop long (${n} caractères au plus)`,
  ],
  [
    /^This sync server doesn't take messages yet$/,
    "Ce serveur de synchronisation ne reçoit pas encore les messages",
  ],
  [
    /^Too many messages were sent from your network: try again later$/,
    "Trop de messages ont été envoyés depuis votre réseau : réessayez plus tard",
  ],

  // IBAN, items and comments
  [/^This IBAN is not valid$/, "Cet IBAN n'est pas valide"],
  [
    /^The items add up to (.*), not the expense's (.*)$/,
    (a, b) => `Les articles totalisent ${a}, pas les ${b} de la dépense`,
  ],
  [/^Each item needs at least one person$/, "Chaque article doit être pour au moins une personne"],
  [/^An item's amount must be above zero$/, "Le montant d'un article doit être supérieur à zéro"],
  [
    /^An item's name is too long \((\d+) characters at most\)$/,
    (n) => `Le nom d'un article est trop long (${n} caractères au plus)`,
  ],
  [
    /^An expense can't have more than (\d+) items$/,
    (n) => `Une dépense ne peut pas avoir plus de ${n} articles`,
  ],
  [/^A participant appears twice on an item$/, "Une personne figure deux fois sur un article"],
  [/^A comment can't be empty$/, "Un commentaire ne peut pas être vide"],
  [
    /^This comment is too long \((\d+) characters at most\)$/,
    (n) => `Ce commentaire est trop long (${n} caractères au plus)`,
  ],
  [/^This comment no longer exists$/, "Ce commentaire n'existe plus"],

  // Edit summaries, kept with the expense
  [/^Updated without major changes$/, "Mise à jour sans changement notable"],
  [/^Restored from the trash$/, "Restaurée depuis la corbeille"],
  [/^Items updated$/, "Articles modifiés"],
  [/^Title changed from '(.*)' to '(.*)'$/, (a, b) => `Titre changé de « ${a} » à « ${b} »`],
  [/^Amount changed from (.*) to (.*)$/, (a, b) => `Montant changé de ${a} à ${b}`],
  [/^Payer changed from (.*) to (.*)$/, (a, b) => `Payeur changé de ${a} à ${b}`],
  [
    /^Category changed from (.*) to (.*)$/,
    (a, b) => `Catégorie changée de « ${category(a)} » à « ${category(b)} »`,
  ],
  [/^Paid in (.*) instead of (.*)$/, (a, b) => `Payé en ${a} au lieu de ${b}`],
  [/^Participants \/ parts allocation updated$/, "Répartition entre les participants modifiée"],
  [/^Date changed from (.*) to (.*)$/, (a, b) => `Date changée du ${a} au ${b}`],

  // Saved data
  [
    /^Group (.*) could not be loaded \((.*)\)\. Its data was kept in the database\.$/s,
    (id, e) =>
      `Le groupe ${id} n'a pas pu être chargé (${e}). Ses données ont été conservées dans la base.`,
  ],
];

const PATTERNS: Partial<Record<Language, Pattern[]>> = { fr: FR };

function translate(text: string, patterns: Pattern[]): string {
  for (const [pattern, replacement] of patterns) {
    const match = pattern.exec(text);
    if (!match) continue;
    return typeof replacement === "string" ? replacement : replacement(...match.slice(1));
  }
  return text;
}

/** A message from the core, in the app's language when it is one this file knows. */
export function backendText(text: string): string {
  const patterns = PATTERNS[i18n.language];
  return patterns ? translate(text, patterns) : text;
}

/** An edit summary: the core joins what changed with "; ". */
export function summaryText(summary: string): string {
  const patterns = PATTERNS[i18n.language];
  if (!patterns) return summary;
  return summary
    .split("; ")
    .map((change) => translate(change, patterns))
    .join(" ; ");
}
