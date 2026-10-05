// What the core refuses that the tests run into: IBANs, pictures, usernames and passwords.

// An IBAN as stored, like doc.rs's `check_iban`.
export function checkIban(typed: string) {
  const iban = typed.replace(/\s/g, "").toUpperCase();
  let rest = 0;
  for (const c of iban.slice(4) + iban.slice(0, 4)) {
    const n = Number.parseInt(c, 36);
    rest = (n > 9 ? rest * 100 + n : rest * 10 + n) % 97;
  }
  if (!/^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(iban) || rest !== 1) {
    throw new Error("This IBAN is not valid");
  }
  return iban;
}

const PICTURE = /^data:image\/(jpeg|png|webp);base64,[A-Za-z0-9+/=]+$/;

export function checkPicture(picture: string | null | undefined) {
  if (picture == null) return;
  if (!PICTURE.test(picture)) {
    throw new Error("This picture can't be used: pick a JPEG, PNG or WebP image");
  }
  if (picture.length > 200_000) throw new Error("This picture is too big");
}

// A stand-in for zxcvbn: longer is stronger, a few common passwords and the username are weak.
export function passwordStrength(password: string, username: string) {
  const pw = String(password || "");
  let score = pw.length < 8 ? 0 : pw.length < 10 ? 1 : pw.length < 12 ? 2 : pw.length < 16 ? 3 : 4;
  let warning: string | null = null;
  if (["password", "password123", "qwertyuiop"].includes(pw.toLowerCase())) {
    score = 0;
    warning = "This is a top-10 common password.";
  }
  const name = String(username || "")
    .trim()
    .toLowerCase();
  if (name && pw.toLowerCase().includes(name)) score = Math.min(score, 1);
  return {
    score,
    acceptable: pw.length >= 8 && score >= 3,
    warning,
    suggestions: score < 3 ? ["Add another word or two. Uncommon words are better."] : [],
  };
}

export function checkCredentials(username: string, password: string, signingUp: boolean) {
  const name = String(username || "")
    .trim()
    .toLowerCase();
  if (!/^[a-z0-9][a-z0-9._-]{2,31}$/.test(name)) {
    throw new Error(
      signingUp
        ? "Usernames are 3 to 32 letters, digits, dots, dashes or underscores"
        : "Wrong username or password"
    );
  }
  if (signingUp && String(password).length < 8) {
    throw new Error("Use a password of at least 8 characters");
  }
  if (signingUp && !passwordStrength(password, name).acceptable) {
    throw new Error("This password is too easy to guess. Try a few unrelated words.");
  }
  return name;
}
