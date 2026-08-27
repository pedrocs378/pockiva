# ROM Test Assets

No commercial ROM may enter the repository, generated artifacts, CI cache, release bundle, or ordinary application download path.

Blargg, Mooneye, and homebrew assets may be used only when redistribution or test use is legally permitted. A future explicit developer download script must record, for every asset:

- project and test-case name;
- upstream source URL;
- license or redistribution terms;
- immutable revision or release tag;
- SHA-256 checksum;
- expected emulator pass/fail signal;
- maximum cycle or wall-clock test bound.

Ordinary install, build, test discovery, and application startup must not silently download ROMs. If redistribution is not permitted, CI obtains the asset explicitly from its documented source and verifies its checksum without committing it. End-to-end tests use a legally redistributable homebrew ROM, never a commercial cartridge dump.
