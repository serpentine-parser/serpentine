/**
 * Utility functions for converting SVG to PNG
 */

export interface SvgToPngOptions {
  width?: number;
  height?: number;
  backgroundColor?: string;
  scale?: number;
}

/**
 * Get all CSS rules that might affect the SVG
 */
function extractRelevantCssRules(): string {
  let cssText = "";

  try {
    // Iterate through all stylesheets
    Array.from(document.styleSheets).forEach((styleSheet) => {
      try {
        // Only process stylesheets from the same origin or those we can access
        const rules = styleSheet.cssRules || styleSheet.rules;
        if (rules) {
          Array.from(rules).forEach((rule) => {
            // Include rules that might affect SVG elements
            if (
              rule.cssText &&
              (rule.cssText.includes("fill-") ||
                rule.cssText.includes("stroke-") ||
                rule.cssText.includes("text-") ||
                rule.cssText.includes("dark:") ||
                rule.cssText.includes("svg") ||
                rule.cssText.includes("path") ||
                rule.cssText.includes("rect") ||
                rule.cssText.includes("circle") ||
                rule.cssText.includes("line") ||
                rule.cssText.includes("g "))
            ) {
              cssText += rule.cssText + "\n";
            }
          });
        }
      } catch (e) {
        // Skip stylesheets we can't access (CORS)
        console.warn("Cannot access stylesheet:", e);
      }
    });
  } catch (e) {
    console.warn("Error extracting CSS rules:", e);
  }

  return cssText;
}

/**
 * Apply computed styles to SVG elements recursively
 */
function applyComputedStylesToSvg(
  element: Element,
  computedStyleMap: Map<Element, CSSStyleDeclaration>
): void {
  const computedStyle = computedStyleMap.get(element);
  if (computedStyle) {
    // Focus on SVG-relevant style properties
    const relevantProps = [
      "fill",
      "stroke",
      "stroke-width",
      "stroke-dasharray",
      "stroke-linecap",
      "stroke-linejoin",
      "opacity",
      "fill-opacity",
      "stroke-opacity",
      "font-family",
      "font-size",
      "font-weight",
      "text-anchor",
      "dominant-baseline",
      "visibility",
      "display",
      "transform",
    ];

    const styleStr = relevantProps
      .filter((prop) => computedStyle.getPropertyValue(prop))
      .map((prop) => `${prop}: ${computedStyle.getPropertyValue(prop)}`)
      .join("; ");

    if (styleStr) {
      element.setAttribute("style", styleStr);
    }
  }

  // Process children
  Array.from(element.children).forEach((child) => {
    applyComputedStylesToSvg(child, computedStyleMap);
  });
}

/**
 * Convert an SVG element to PNG and download it
 */
export function exportSvgToPng(
  svgElement: SVGSVGElement,
  filename: string = "serpentine-graph.png",
  options: SvgToPngOptions = {}
): Promise<void> {
  return new Promise((resolve, reject) => {
    try {
      // Get the SVG dimensions
      const svgRect = svgElement.getBoundingClientRect();
      const svgWidth = options.width || svgRect.width;
      const svgHeight = options.height || svgRect.height;
      const scale = options.scale || 2; // Default to 2x for high resolution

      // Create a canvas element
      const canvas = document.createElement("canvas");
      const ctx = canvas.getContext("2d");

      if (!ctx) {
        reject(new Error("Failed to get canvas context"));
        return;
      }

      // Set canvas dimensions (scaled for high resolution)
      canvas.width = svgWidth * scale;
      canvas.height = svgHeight * scale;

      // Set the background color if specified
      if (options.backgroundColor) {
        ctx.fillStyle = options.backgroundColor;
        ctx.fillRect(0, 0, canvas.width, canvas.height);
      }

      // Clone the SVG to avoid modifying the original
      const svgClone = svgElement.cloneNode(true) as SVGSVGElement;

      // Set explicit dimensions on the clone
      svgClone.setAttribute("width", svgWidth.toString());
      svgClone.setAttribute("height", svgHeight.toString());

      // Collect computed styles for all elements in the original SVG
      const computedStyleMap = new Map<Element, CSSStyleDeclaration>();
      const collectComputedStyles = (
        element: Element,
        clonedElement: Element
      ) => {
        computedStyleMap.set(clonedElement, window.getComputedStyle(element));

        // Process children
        Array.from(element.children).forEach((child, index) => {
          const clonedChild = clonedElement.children[index];
          if (clonedChild) {
            collectComputedStyles(child, clonedChild);
          }
        });
      };

      collectComputedStyles(svgElement, svgClone);

      // Apply computed styles to the cloned SVG
      applyComputedStylesToSvg(svgClone, computedStyleMap);

      // Extract relevant CSS rules and embed them in the SVG
      const cssRules = extractRelevantCssRules();
      if (cssRules) {
        const styleElement = document.createElementNS(
          "http://www.w3.org/2000/svg",
          "style"
        );
        styleElement.textContent = cssRules;
        svgClone.insertBefore(styleElement, svgClone.firstChild);
      }

      // Convert SVG to data URL
      const svgData = new XMLSerializer().serializeToString(svgClone);
      const svgBlob = new Blob([svgData], {
        type: "image/svg+xml;charset=utf-8",
      });
      const url = URL.createObjectURL(svgBlob);

      // Create an image element and load the SVG
      const img = new Image();
      img.onload = () => {
        try {
          // Scale the context for high resolution
          ctx.scale(scale, scale);

          // Draw the image to canvas
          ctx.drawImage(img, 0, 0, svgWidth, svgHeight);

          // Convert canvas to blob
          canvas.toBlob((blob) => {
            if (!blob) {
              reject(new Error("Failed to create PNG blob"));
              return;
            }

            // Create download link
            const downloadUrl = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = downloadUrl;
            a.download = filename;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);

            // Cleanup
            URL.revokeObjectURL(url);
            URL.revokeObjectURL(downloadUrl);
            resolve();
          }, "image/png");
        } catch (error) {
          reject(error);
        }
      };

      img.onerror = () => {
        URL.revokeObjectURL(url);
        reject(new Error("Failed to load SVG image"));
      };

      img.src = url;
    } catch (error) {
      reject(error);
    }
  });
}

/**
 * Get the current theme background color for PNG export
 */
export function getThemeBackgroundColor(): string {
  // Check if dark mode is active
  const isDark =
    document.documentElement.classList.contains("dark") ||
    window.matchMedia("(prefers-color-scheme: dark)").matches;

  return isDark ? "#0f172a" : "#ffffff"; // slate-900 for dark, white for light
}
