import readme from "../../../README.md?raw";

/**
 * Install methods, parsed from the Installation section of the root README so
 * the site cannot drift from the documented commands.
 *
 * Shared by the hero picker and the closing call to action — both render the
 * same list, so they are parsed once here rather than in each component.
 */
function parseInstallMethods(md) {
  const section = md.split("## Installation")[1]?.split(/\n## [^#]/)[0];
  if (!section) return [];

  const methods = [];
  const regex = /### (.+)\n[\s\S]*?```sh\n(.+)\n```/g;
  let match;

  while ((match = regex.exec(section)) !== null) {
    const label = match[1].trim();
    methods.push({
      id: label.toLowerCase().replace(/\s+/g, "-"),
      label,
      command: match[2].trim(),
    });
  }

  return methods;
}

export const installMethods = parseInstallMethods(readme);

/** Short labels for the hero picker, which has less room than the CTA list. */
const shortLabels = {
  "install-script": "Script",
  "install-from-source": "Cargo",
};

export const shortLabel = (method) => shortLabels[method.id] ?? method.label;
