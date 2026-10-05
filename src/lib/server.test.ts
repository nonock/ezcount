import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SERVER, privacyUrl, rememberedServer, rememberServer } from "./server";

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("the relay the login screen starts with", () => {
  it("is the default one until another is used", () => {
    expect(rememberedServer()).toBe(DEFAULT_SERVER);
  });

  it("is the one last logged into", () => {
    rememberServer("https://relay.example.com");
    expect(rememberedServer()).toBe("https://relay.example.com");
  });

  it("goes back to the default without keeping it, so a new default is followed", () => {
    rememberServer("https://relay.example.com");
    rememberServer(DEFAULT_SERVER);
    expect(rememberedServer()).toBe(DEFAULT_SERVER);
    expect(localStorage.length).toBe(0);
  });

  it("is the default where nothing can be remembered", () => {
    const refuse = () => {
      throw new Error("storage is off");
    };
    vi.stubGlobal("localStorage", { getItem: refuse, setItem: refuse, removeItem: refuse });
    expect(() => rememberServer("https://relay.example.com")).not.toThrow();
    expect(rememberedServer()).toBe(DEFAULT_SERVER);
  });
});

describe("a relay's privacy policy", () => {
  it("is its own page, in the app's language", () => {
    expect(privacyUrl("https://relay.example.com", "fr")).toBe(
      "https://relay.example.com/privacy?lang=fr"
    );
  });

  it("is found whatever ends the address", () => {
    expect(privacyUrl(" https://relay.example.com/ ", "en")).toBe(
      "https://relay.example.com/privacy?lang=en"
    );
  });
});
