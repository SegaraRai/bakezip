<script lang="ts">
  import type {
    CompatibilityLevel,
    EncodingSelectionStrategy,
    FieldSelectionStrategy,
    InspectedArchive,
    WaveDashHandling,
    WaveDashNormalization,
  } from "bakezip";
  import { onMount } from "svelte";
  import LineMdAlert from "../icons/LineMdAlert.svelte";
  import LineMdAlertCircle from "../icons/LineMdAlertCircle.svelte";
  import LineMdCloseCircle from "../icons/LineMdCloseCircle.svelte";
  import LineMdConfirmCircle from "../icons/LineMdConfirmCircle.svelte";
  import LineMdDownload from "../icons/LineMdDownload.svelte";
  import LineMdDownloadLoop from "../icons/LineMdDownloadLoop.svelte";
  import LineMdFolderZip from "../icons/LineMdFolderZip.svelte";
  import LineMdLoadingLoop from "../icons/LineMdLoadingLoop.svelte";
  import {
    initializeWorker,
    parseZipInWorker,
    type ZipProcessorProxy,
  } from "../lib/bakezipWorkerClient";
  import { createI18n, type Locale } from "../lib/i18n";

  const { locale }: { locale: Locale } = $props();
  const m = $derived.by(() => createI18n(locale));

  onMount(() => {
    requestIdleCallback(async () => {
      await initializeWorker();
    });
  });

  let selectedFile = $state<File | null>(null);
  let processor = $state.raw<ZipProcessorProxy | null>(null);
  let inspectedArchive = $state.raw<InspectedArchive | null>(null);

  // These should ideally be derived, but `$derived` is too slow to inspect many entries
  let decodeErrorCount = $state<number>(0);
  let hasOSMetadataFiles = $state<boolean>(false);

  let busy = $state<"parsing" | "inspecting" | "rebuilding" | false>(false);
  let error = $state("");

  const newFilename = $derived.by(() => {
    if (!selectedFile) {
      return "bakezip.zip";
    }

    const name = selectedFile.name;
    const dotIndex = name.lastIndexOf(".");
    if (dotIndex === -1) {
      return `${name}_bakezip.zip`;
    } else {
      const base = name.substring(0, dotIndex);
      const ext = name.substring(dotIndex);
      return `${base}_bakezip${ext}`;
    }
  });

  const compatibility = $derived.by(
    (): CompatibilityLevel | null => processor?.compatibility ?? null,
  );
  const warnings = $derived.by(() => processor?.warnings ?? []);

  const waveDashOptions = $derived.by(() => {
    if (!inspectedArchive) {
      return null;
    }

    if (
      inspectedArchive.contains_sjis_wave_dash &&
      (inspectedArchive.contains_other_wave_dash ||
        inspectedArchive.contains_other_fullwidth_tilde)
    ) {
      return "both";
    }

    if (inspectedArchive.contains_sjis_wave_dash) {
      return "shift_jis";
    }

    if (
      inspectedArchive.contains_other_wave_dash ||
      inspectedArchive.contains_other_fullwidth_tilde
    ) {
      return "non_shift_jis";
    }

    return null;
  });

  // Options for Step2
  let encoding = $state("__PreferOverallDetected");
  let fieldSelection = $state<FieldSelectionStrategy>(
    "CdhUnicodeThenLfhUnicodeThenCdh",
  );
  let waveDashHandling = $state<WaveDashHandling>("DecodeToFullwidthTilde");
  let waveDashNormalization = $state<WaveDashNormalization>("Preserve");
  let forceProceedToStep2 = $state(false);
  let expandStep2 = $state(false);

  // Options for Step3
  let removeOSMetadataFiles = $state(false);
  let forceProceedToStep3 = $state(false);

  // Download states
  let downloaded = $state(false);
  let downloadFile = $state<File | null>(null);

  let _downloadURL = $state<string | null>(null);
  const downloadURL = $derived.by(() => _downloadURL);

  $effect(() => {
    if (!downloadFile) {
      _downloadURL = null;
      downloaded = false;
      return;
    }

    const url = URL.createObjectURL(downloadFile);
    _downloadURL = url;
    downloaded = false;

    return () => {
      URL.revokeObjectURL(url);
      downloaded = false;
    };
  });

  const step1Complete = $derived.by(() => {
    return processor != null && compatibility != null;
  });

  const step2Complete = $derived.by(() => {
    return step1Complete && inspectedArchive != null;
  });

  const compatibilityCategory = $derived.by(() => {
    switch (compatibility?.type) {
      case "AsciiOnly":
        return {
          level: "ok",
          message:
            compatibility.with_utf8_flags === "Always"
              ? "compatibility_ascii_only_with_flag"
              : "compatibility_ascii_only",
        } as const;

      case "Utf8Only": {
        const withFlags =
          compatibility.with_utf8_flags === "Always" ||
          compatibility.with_utf8_flags === "AlwaysForNonAscii";
        return {
          level: withFlags ? "ok" : "other",
          message: withFlags
            ? "compatibility_utf8_only"
            : "compatibility_no_utf8_flag",
        } as const;
      }

      case "Other":
        return {
          level: "other",
          message:
            compatibility.with_unicode_path_fields === "Always" ||
            compatibility.with_unicode_path_fields === "AlwaysForNonAscii"
              ? "compatibility_other_with_unicode_path"
              : "compatibility_other",
        } as const;

      case "Broken":
        return {
          level: "broken",
          message: "compatibility_broken",
        } as const;

      default:
        return {
          level: "other",
          message: "compatibility_unknown",
        } as const;
    }
  });

  const shouldPauseAtStep1 = $derived.by(
    () => compatibilityCategory.level === "ok",
  );

  const shouldPauseAtStep2 = $derived.by(() => decodeErrorCount > 0);

  const formatBytes = $derived.by(() => {
    const K = 1024;

    const sizes = [
      m.bytes_unit_b(),
      m.bytes_unit_kb(),
      m.bytes_unit_mb(),
      m.bytes_unit_gb(),
    ];

    return (bytes: number): string => {
      if (bytes <= 0) {
        return `0 ${sizes[0]}`;
      }
      const i = Math.floor(Math.log(bytes) / Math.log(K));
      const s = Math.round((bytes / Math.pow(K, i)) * 100) / 100;
      return `${s} ${sizes[i]}`;
    };
  });

  function isOSMetadataFile(filename: string): boolean {
    return (
      // directories and files
      /(?:^|\/)(?:__macosx\/|\.ds_store)(?:\/|$)/i.test(filename) ||
      // files
      /(?:^|\/)(?:thumbs\.db|desktop\.ini)$/i.test(filename)
    );
  }

  async function handleFileSelect(event: Event) {
    if (busy) {
      return;
    }

    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];

    if (!file) {
      return;
    }

    selectedFile = file;
    error = "";

    // Automatically process the ZIP file
    await processZip();
  }

  function waitUITick(): Promise<void> {
    return new Promise((resolve) => requestAnimationFrame(() => resolve()));
  }

  async function resetStates(step: 1 | 2 | 3): Promise<void> {
    error = "";

    if (step <= 1) {
      // Dispose previous processor if any
      await processor?.dispose();
      processor = null;

      forceProceedToStep2 = false;
    }

    if (step <= 2) {
      inspectedArchive = null;
      decodeErrorCount = 0;
      hasOSMetadataFiles = false;

      expandStep2 = false;
      forceProceedToStep3 = false;
      encoding = "__PreferOverallDetected";
      fieldSelection = "CdhUnicodeThenLfhUnicodeThenCdh";
    }

    if (step <= 3) {
      removeOSMetadataFiles = false;
      downloaded = false;
      downloadFile = null;
    }
  }

  function getFinalWaveDashOptions(
    archive: InspectedArchive,
  ): readonly [WaveDashHandling, WaveDashNormalization] {
    if (
      archive.contains_other_wave_dash ||
      archive.contains_other_fullwidth_tilde
    ) {
      // If the archive contains non-SJIS wave dash or fullwidth tilde, we must use waveDashNormalization setting
      return [
        waveDashNormalization === "NormalizeToWaveDash"
          ? "DecodeToWaveDash"
          : "DecodeToFullwidthTilde",
        waveDashNormalization,
      ];
    }

    // Otherwise, we can use waveDashHandling setting
    // WaveDashNormalization can be any value since it won't affect anything
    return [waveDashHandling, "Preserve"];
  }

  async function processZip() {
    if (!selectedFile) {
      error = m.step1_error_no_file();
      return;
    }

    busy = "parsing";
    await resetStates(1);
    await waitUITick();

    try {
      // Parse the ZIP file
      const ts = performance.now();
      const result = await parseZipInWorker(selectedFile);
      const elapsed = performance.now() - ts;
      console.info(`Parsed ${selectedFile.name} in ${elapsed} ms`);

      // Ensure minimum loading time for UX
      if (elapsed < 300) {
        await new Promise((resolve) =>
          setTimeout(resolve, Math.floor(300 - elapsed)),
        );
      }

      processor = result;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      processor = null;
    } finally {
      busy = false;
    }

    if (processor) {
      await inspectArchive();
    }
  }

  async function inspectArchive(): Promise<void> {
    if (!processor) {
      return;
    }

    const isNew = inspectedArchive == null;

    busy = "inspecting";
    if (isNew) {
      await resetStates(2);
    }
    await waitUITick();

    try {
      const ts = performance.now();
      const result = await processor.inspect({
        encoding: getEncodingStrategy(encoding),
        field_selection_strategy: fieldSelection,
        ignore_crc32_mismatch: false,
        needs_original_bytes: false,
        // During inspection, we have to use fixed options to detect wave dash correctly
        wave_dash_handling: "DecodeToFullwidthTilde",
        wave_dash_normalization: "Preserve",
      });
      const elapsed = performance.now() - ts;
      console.info(`Inspected archive in ${elapsed} ms`);

      forceProceedToStep3 = false;

      if (isNew) {
        expandStep2 =
          result.overall_encoding == null ||
          result.entries.some(
            (entry) => entry.filename.decoded?.has_errors !== false,
          );
      }

      if (
        result.contains_sjis_wave_dash &&
        (result.contains_other_wave_dash ||
          result.contains_other_fullwidth_tilde) &&
        waveDashNormalization === "Preserve"
      ) {
        // If the archive contains both SJIS wave dash and other wave dash/fullwidth tilde, we cannot use "Preserve" mode
        // since we need to select either character for SJIS entries.
        waveDashNormalization = "NormalizeToFullwidthTilde";
      }

      if (
        !result.contains_sjis_wave_dash &&
        (result.contains_other_wave_dash ||
          result.contains_other_fullwidth_tilde)
      ) {
        if (
          waveDashNormalization === "NormalizeToFullwidthTilde" &&
          !result.contains_other_wave_dash
        ) {
          // If there is no SJIS wave dash, and there is no non-SJIS wave dash,
          // we cannot use "NormalizeToFullwidthTilde" mode since it has no effect.
          waveDashNormalization = "Preserve";
        }

        if (
          waveDashNormalization === "NormalizeToWaveDash" &&
          !result.contains_other_fullwidth_tilde
        ) {
          // If there is no SJIS wave dash, and there is no non-SJIS fullwidth tilde,
          // we cannot use "NormalizeToWaveDash" mode since it has no effect.
          waveDashNormalization = "Preserve";
        }
      }

      decodeErrorCount = result.entries.reduce(
        (count, entry) =>
          entry.filename.decoded?.has_errors !== false ? count + 1 : count,
        0,
      );

      hasOSMetadataFiles = result.entries.some(
        (entry) =>
          entry.filename.decoded &&
          isOSMetadataFile(entry.filename.decoded.string),
      );

      inspectedArchive = result;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      inspectedArchive = null;
    } finally {
      busy = false;
    }

    if (inspectedArchive) {
      await rebuildArchive();
    }
  }

  async function rebuildArchive() {
    if (!processor || !inspectedArchive) {
      return;
    }

    const isNew = downloadFile == null;

    busy = "rebuilding";
    if (isNew) {
      await resetStates(3);
    }
    await waitUITick();

    try {
      const ts = performance.now();
      const omitEntries = removeOSMetadataFiles
        ? inspectedArchive.entries
            .map((entry, index) =>
              isOSMetadataFile(entry.filename.decoded?.string ?? "")
                ? BigInt(index)
                : null,
            )
            .filter((index): index is bigint => index != null)
        : [];

      const [finalWaveDashHandling, finalWaveDashNormalization] =
        getFinalWaveDashOptions(inspectedArchive);

      const rebuiltBlob = await processor.rebuild(
        {
          encoding: getEncodingStrategy(encoding),
          field_selection_strategy: fieldSelection,
          ignore_crc32_mismatch: false,
          needs_original_bytes: false,
          wave_dash_handling: finalWaveDashHandling,
          wave_dash_normalization: finalWaveDashNormalization,
        },
        new BigUint64Array(omitEntries),
      );
      const elapsed = performance.now() - ts;
      console.info(`Rebuilt archive in ${elapsed} ms`);

      downloadFile = new File([rebuiltBlob], newFilename, {
        type: "application/zip",
      });
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      downloadFile = null;
    } finally {
      busy = false;
    }
  }

  function getEncodingStrategy(
    encodingValue: string,
  ): EncodingSelectionStrategy {
    if (encodingValue === "__PreferOverallDetected") {
      return {
        type: "PreferOverallDetected",
        fallback_encoding: undefined,
        ignore_utf8_flag: false,
      };
    } else if (encodingValue === "__EntryDetected") {
      return {
        type: "EntryDetected",
        fallback_encoding: undefined,
        ignore_utf8_flag: false,
      };
    } else {
      return {
        type: "ForceSpecified",
        encoding: encodingValue,
        ignore_utf8_flag: true,
      };
    }
  }

  function handleDownload() {
    downloaded = true;
  }
</script>

<div class="p-4 md:p-8">
  <div class="mx-auto max-w-6xl">
    <!-- Header -->
    <div class="mb-8 text-center [view-transition-name:app-header]">
      <h1 class="mb-2 text-4xl font-bold text-base-content">
        {m.header_title()}
      </h1>
      <p class="text-lg text-base-content/80">
        {m.header_tagline()}
      </p>
      <p class="mt-2 text-sm text-base-content/60">
        {m.header_description()}
      </p>
    </div>

    <!-- Main Cards -->
    <div class="space-y-8">
      <!-- Step 1: ZIP Select and Compatibility Display -->
      <div
        id="step1"
        role="region"
        aria-labelledby="step1-title"
        class="card bg-base-100 shadow-xl [view-transition-name:step1]"
      >
        <div class="card-body">
          <div id="step1-title" class="mb-4 flex items-center gap-3">
            <span
              aria-label={m.step1_alt()}
              class="badge badge-primary badge-lg text-center size-8 rounded-full p-0 font-bold before:content-[attr(data-step)]"
              data-step="1"
            ></span>
            <h2 class="card-title text-2xl">{m.step1_title()}</h2>
          </div>

          <!-- File Select Section -->
          <div class="mb-4">
            <p class="mb-3 text-sm text-base-content/70">
              {m.step1_info()}
            </p>
            <div
              class="relative flex items-center justify-center rounded-lg border-2 border-dashed border-primary/30 bg-base-200 px-3 py-6 cursor-wait has-focus-visible:border-primary has-enabled:cursor-pointer has-enabled:hover:border-primary has-enabled:hover:bg-base-300 data-[selected=false]:anim-ripple"
              data-selected={!!selectedFile}
            >
              <input
                type="file"
                accept=".zip"
                disabled={!!busy}
                aria-label={m.step1_file_placeholder()}
                class="absolute inset-0 cursor-wait enabled:cursor-pointer opacity-0 appearance-none"
                onchange={handleFileSelect}
              />
              <div class="grid grid-cols-1 place-items-center text-center">
                <LineMdFolderZip
                  aria-hidden="true"
                  class="text-base-content size-14"
                />
                {#if selectedFile}
                  <div lang="" class="text-base-content/70 pt-2">
                    {selectedFile.name}
                  </div>
                  <div class="pt-1 text-sm text-base-content/50">
                    {formatBytes(selectedFile.size)}
                  </div>
                {:else}
                  <p class="text-base-content/70 pt-2">
                    {m.step1_file_placeholder()}
                  </p>
                {/if}
              </div>
            </div>
          </div>

          {#if busy === "parsing"}
            <div role="status" class="alert alert-info" aria-live="polite">
              <LineMdLoadingLoop
                aria-hidden="true"
                class="size-10 motion-reduce:hidden"
              />
              <span
                aria-hidden="true"
                class="size-10 icon-[mdi--hourglass] not-motion-reduce:hidden"
              ></span>
              <p>{m.step1_processing()}</p>
            </div>
          {/if}

          <!-- Error Message -->
          {#if error}
            <div role="alert" class="alert alert-error">
              <LineMdCloseCircle class="size-10" aria-hidden="true" />
              <p lang="en">{error}</p>
            </div>
          {/if}

          <!-- Warnings -->
          {#if warnings.length > 0}
            <div
              role="status"
              aria-live="polite"
              class="alert alert-soft alert-warning flex-col items-start"
            >
              <LineMdAlert class="size-10" aria-hidden="true" />
              <div class="w-full">
                <h3 class="font-bold mb-1">
                  {m.step1_warnings_title({ count: warnings.length })}
                </h3>
                <div class="overflow-y-auto max-h-38">
                  <ul class="list-disc list-inside text-sm">
                    {#each warnings as warning}
                      <li>
                        {#if warning.index !== null && warning.index !== undefined}
                          <span class="[font-feature-settings:tnum]">
                            {m.step1_warning_entry_prefix({
                              index: warning.index,
                            })}
                          </span>
                        {/if}
                        <span lang="en">{warning.message}</span>
                      </li>
                    {/each}
                  </ul>
                </div>
              </div>
            </div>
          {/if}

          <!-- Compatibility Check Results -->
          {#if step1Complete && compatibility}
            <div
              role="status"
              aria-live="polite"
              class="alert data-[compatibility=ok]:alert-success data-[compatibility=broken]:alert-error data-[compatibility=other]:alert-info"
              data-compatibility={compatibilityCategory.level}
            >
              {#if compatibilityCategory.level === "ok"}
                <LineMdConfirmCircle aria-hidden="true" class="size-10" />
              {:else if compatibilityCategory.level === "broken"}
                <LineMdCloseCircle aria-hidden="true" class="size-10" />
              {:else}
                <LineMdAlertCircle aria-hidden="true" class="size-10" />
              {/if}
              <div>
                <h3 class="font-bold">{m.compatibility_title()}</h3>
                <p class="text-sm">
                  {m[compatibilityCategory.message]()}
                </p>
              </div>
            </div>
          {/if}

          {#if shouldPauseAtStep1}
            <div class="mt-2">
              <label class="label">
                <input
                  aria-controls="step2"
                  aria-expanded={forceProceedToStep2}
                  name="force-step2"
                  type="checkbox"
                  class="peer checkbox"
                  bind:checked={forceProceedToStep2}
                />
                <span class="peer-checked:text-primary"
                  >{m.step1_process_anyway()}</span
                >
              </label>
            </div>
          {/if}
        </div>
      </div>

      <!-- Step 2: Configuration (Optional, if needed) -->
      {#if step1Complete && (!shouldPauseAtStep1 || forceProceedToStep2)}
        <div
          id="step2"
          role="region"
          aria-labelledby="step2-title"
          class="card bg-base-100 shadow-xl [view-transition-name:step2]"
        >
          <div class="card-body">
            <button
              aria-controls="step2-body"
              aria-expanded={expandStep2}
              class="flex w-full items-center gap-3 text-left"
              onclick={() => {
                expandStep2 = !expandStep2;
              }}
            >
              <div id="step2-title" class="contents">
                <span
                  aria-label={m.step2_alt()}
                  class="badge badge-primary badge-lg text-center size-8 rounded-full p-0 font-bold before:content-[attr(data-step)]"
                  data-step="2"
                ></span>
                <h2 class="card-title flex-1 text-2xl">{m.step2_title()}</h2>
              </div>
              <span
                aria-hidden="true"
                class="icon-[mdi--chevron-down] size-8 not-motion-reduce:transition-transform data-[expanded=true]:rotate-180"
                data-expanded={expandStep2}
              ></span>
            </button>

            <div
              id="step2-body"
              class="group space-y-6 pt-4"
              data-expanded={expandStep2}
            >
              <!-- Encoding Selection -->
              <fieldset class="fieldset group-data-[expanded=false]:hidden">
                <legend class="fieldset-legend text-sm"
                  >{m.step2_encoding_label()}</legend
                >
                <select
                  name="encoding-select"
                  class="select w-full"
                  bind:value={encoding}
                  onchange={inspectArchive}
                >
                  <option value="__PreferOverallDetected"
                    >{m.step2_encoding_auto_overall()}</option
                  >
                  <option value="__EntryDetected"
                    >{m.step2_encoding_auto_entry()}</option
                  >
                  <option value="UTF-8">{m.step2_encoding_force_utf8()}</option>
                  <option value="Shift_JIS"
                    >{m.step2_encoding_force_shift_jis()}</option
                  >
                  <option value="EUC-KR"
                    >{m.step2_encoding_force_euc_kr()}</option
                  >
                  <option value="GBK">{m.step2_encoding_force_gbk()}</option>
                  <option value="Big5">{m.step2_encoding_force_big5()}</option>
                </select>
                <p class="label">
                  {m.step2_encoding_force_note()}
                </p>
              </fieldset>

              <!-- Field Selection -->
              <fieldset class="fieldset group-data-[expanded=false]:hidden">
                <legend class="fieldset-legend text-sm"
                  >{m.step2_field_selection_label()}</legend
                >
                <select
                  name="field-select"
                  class="select w-full"
                  bind:value={fieldSelection}
                  onchange={inspectArchive}
                >
                  <option value="CdhUnicodeThenLfhUnicodeThenCdh"
                    >{m.step2_field_cdh_unicode_lfh_unicode_cdh()}</option
                  >
                  <option value="CdhUnicodeThenLfhUnicodeThenLfh"
                    >{m.step2_field_cdh_unicode_lfh_unicode_lfh()}</option
                  >
                  <option value="LfhUnicodeThenCdhUnicodeThenCdh"
                    >{m.step2_field_lfh_unicode_cdh_unicode_cdh()}</option
                  >
                  <option value="LfhUnicodeThenCdhUnicodeThenLfh"
                    >{m.step2_field_lfh_unicode_cdh_unicode_lfh()}</option
                  >
                  <option value="CdhUnicodeThenCdh"
                    >{m.step2_field_cdh_unicode_cdh()}</option
                  >
                  <option value="LfhUnicodeThenLfh"
                    >{m.step2_field_lfh_unicode_lfh()}</option
                  >
                  <option value="CdhOnly">{m.step2_field_cdh_only()}</option>
                  <option value="LfhOnly">{m.step2_field_lfh_only()}</option>
                </select>
                <p class="label">
                  {m.step2_field_selection_note()}
                </p>
              </fieldset>

              <!-- Wave Dash Options -->
              {#if waveDashOptions === "shift_jis"}
                <fieldset class="fieldset group-data-[expanded=false]:hidden">
                  <legend class="fieldset-legend text-sm">
                    {m.option_wave_dash_handling()}
                  </legend>
                  <select
                    class="select w-full"
                    bind:value={waveDashHandling}
                    onchange={inspectArchive}
                    disabled={!!busy}
                  >
                    <option value="DecodeToFullwidthTilde">
                      {m.option_wd_fullwidth_tilde_default()}
                    </option>
                    <option value="DecodeToWaveDash">
                      {m.option_wd_wave_dash()}
                    </option>
                  </select>
                </fieldset>
              {:else if waveDashOptions === "non_shift_jis"}
                <fieldset class="fieldset group-data-[expanded=false]:hidden">
                  <legend class="fieldset-legend text-sm">
                    {m.option_wave_dash_normalization()}
                  </legend>
                  <select
                    class="select w-full"
                    bind:value={waveDashNormalization}
                    onchange={inspectArchive}
                    disabled={!!busy}
                  >
                    <option value="Preserve">
                      {m.option_wd_preserve_default()}
                    </option>
                    <option
                      value="NormalizeToFullwidthTilde"
                      disabled={!inspectedArchive?.contains_other_wave_dash}
                    >
                      {m.option_wd_unify_fullwidth_tilde()}
                    </option>
                    <option
                      value="NormalizeToWaveDash"
                      disabled={!inspectedArchive?.contains_other_fullwidth_tilde}
                    >
                      {m.option_wd_unify_wave_dash()}
                    </option>
                  </select>
                </fieldset>
              {:else if waveDashOptions === "both"}
                <fieldset class="fieldset group-data-[expanded=false]:hidden">
                  <legend class="fieldset-legend text-sm">
                    {m.option_wave_dash_normalization()}
                  </legend>
                  <select
                    class="select w-full"
                    bind:value={waveDashNormalization}
                    onchange={inspectArchive}
                    disabled={!!busy}
                  >
                    <option value="NormalizeToFullwidthTilde">
                      {m.option_wd_unify_fullwidth_tilde_default()}
                    </option>
                    <option value="NormalizeToWaveDash">
                      {m.option_wd_unify_wave_dash()}
                    </option>
                  </select>
                </fieldset>
              {/if}

              <!-- Decoded Filenames Preview -->
              {#if inspectedArchive}
                <div>
                  <h3
                    class="mb-3 text-lg font-semibold group-data-[expanded=false]:hidden"
                  >
                    {m.step2_decoded_filenames_title()}
                  </h3>

                  {#if inspectedArchive.overall_encoding}
                    <div class="alert alert-success py-2">
                      <LineMdConfirmCircle aria-hidden="true" class="size-10" />
                      <div>
                        <h3 class="font-bold">
                          {m.step2_detected_encoding_title()}
                        </h3>
                        <p class="text-lg font-bold">
                          {inspectedArchive.overall_encoding}
                        </p>
                      </div>
                    </div>
                  {:else}
                    <div class="alert alert-warning py-2">
                      <LineMdAlert aria-hidden="true" class="size-10" />
                      <div>
                        <h3 class="font-bold">
                          {m.step2_detected_encoding_title()}
                        </h3>
                        <p class="text-sm">
                          {m.step2_detected_encoding_none()}
                        </p>
                      </div>
                    </div>
                  {/if}

                  <div
                    class="group-data-[expanded=false]:hidden max-h-[calc(100vh-16rem)] mt-4 overflow-auto rounded-lg border border-base-200"
                  >
                    <table
                      class="table table-zebra table-pin-rows w-full contain-content [container-name:table]"
                    >
                      <thead class="bg-base-200">
                        <tr>
                          <th scope="col"></th>
                          <th scope="col">{m.step2_table_filename()}</th>
                          <th scope="col"
                            >{m.step2_table_detected_encoding()}</th
                          >
                          <th scope="col">{m.step2_table_field_type()}</th>
                          <th scope="col" class="text-center"
                            >{m.step2_table_utf8_flag()}</th
                          >
                        </tr>
                      </thead>
                      <tbody>
                        {#each inspectedArchive.entries as entry}
                          <tr
                            class="group"
                            data-category={entry.filename.decoded
                              ?.has_errors !== false
                              ? "error"
                              : isOSMetadataFile(entry.filename.decoded.string)
                                ? "metadata"
                                : "default"}
                            data-type={entry.filename.decoded?.string.endsWith(
                              "/",
                            )
                              ? "directory"
                              : "file"}
                            data-encoding-mismatch={entry.filename.decoded
                              ?.encoding_used !==
                              entry.filename.detected_encoding &&
                              entry.filename.detected_encoding !== "ASCII"}
                          >
                            <td class="w-8">
                              <span
                                aria-hidden="true"
                                class="grid place-items-center"
                              >
                                <span
                                  class="text-lg group-data-[type=directory]:group-data-[category=default]:icon-[mdi--folder] group-data-[type=file]:group-data-[category=default]:icon-[mdi--file] group-data-[type=directory]:group-data-[category=metadata]:icon-[mdi--folder-cog] group-data-[type=file]:group-data-[category=metadata]:icon-[mdi--file-cog] group-data-[type=directory]:group-data-[category=alert]:icon-[mdi--folder-alert] group-data-[type=file]:group-data-[category=error]:icon-[mdi--file-alert]"
                                ></span>
                              </span>
                            </td>
                            <td
                              lang={entry.filename.decoded ? "" : undefined}
                              title={entry.filename.decoded?.string}
                              class="min-w-40 max-w-[max(100cqw-20rem,10rem)] truncate group-data-[category=error]:text-error group-data-[category=metadata]:text-info"
                            >
                              {#if entry.filename.decoded}
                                {entry.filename.decoded.string}
                              {:else}
                                <span class="italic">
                                  {m.step2_unable_to_decode()}
                                </span>
                              {/if}
                            </td>
                            <td
                              class="group-data-[encoding-mismatch=true]:text-warning whitespace-nowrap"
                            >
                              <span
                                lang={entry.filename.decoded ? "en" : undefined}
                              >
                                {entry.filename.decoded?.encoding_used ??
                                  m.step2_table_encoding_na()}
                              </span>
                              {#if entry.filename.detected_encoding && entry.filename.detected_encoding !== entry.filename.decoded?.encoding_used}
                                <span lang="en">
                                  ({entry.filename.detected_encoding})
                                </span>
                              {/if}
                            </td>
                            <td class="text-base-content/70 whitespace-nowrap">
                              {m[`field_type_${entry.filename.kind}`]()}
                            </td>
                            <td class="text-center whitespace-nowrap">
                              <span class="grid place-items-center">
                                {#if entry.filename.utf8_flag}
                                  <span
                                    aria-label={m.step2_table_utf8_flag_yes()}
                                    class="icon-[mdi--check] text-success"
                                  ></span>
                                {:else}
                                  <span
                                    aria-label={m.step2_table_utf8_flag_no()}
                                    class="icon-[mdi--minus] text-base-content/30"
                                  ></span>
                                {/if}
                              </span>
                            </td>
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>

                  <!-- Summary Stats -->
                  <div class="stats shadow mt-4 w-full bg-base-200">
                    <div class="stat">
                      <div class="stat-title">
                        {m.step2_stats_total_entries()}
                      </div>
                      <div class="stat-value">
                        {inspectedArchive.entries.length}
                      </div>
                    </div>
                    {#if decodeErrorCount > 0}
                      <div class="stat">
                        <div class="stat-title">
                          {m.step2_stats_decoding_errors()}
                        </div>
                        <div class="stat-value text-error">
                          {decodeErrorCount}
                        </div>
                      </div>
                    {/if}
                  </div>
                </div>
              {/if}
            </div>

            {#if shouldPauseAtStep2}
              <div class="mt-4">
                <label class="label">
                  <input
                    aria-controls="step3"
                    aria-expanded={forceProceedToStep3}
                    name="force-step3"
                    type="checkbox"
                    class="peer checkbox"
                    bind:checked={forceProceedToStep3}
                  />
                  <span class="peer-checked:text-primary"
                    >{m.step2_ignore_errors()}</span
                  >
                </label>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Step 3: Convert and Download -->
      {#if step2Complete && (!shouldPauseAtStep1 || forceProceedToStep2) && (!shouldPauseAtStep2 || forceProceedToStep3)}
        <div
          id="step3"
          role="region"
          aria-labelledby="step3-title"
          class="card bg-base-100 shadow-xl [view-transition-name:step3]"
        >
          <div class="card-body">
            <div id="step3-title" class="mb-4 flex items-center gap-3">
              <span
                aria-label={m.step3_alt()}
                class="badge badge-primary badge-lg text-center size-8 rounded-full p-0 font-bold before:content-[attr(data-step)]"
                data-step="3"
              ></span>
              <h2 class="card-title text-2xl">{m.step3_title()}</h2>
            </div>

            <div class="space-y-4">
              {#if hasOSMetadataFiles}
                <div class="space-y-2">
                  <div
                    role="status"
                    aria-live="polite"
                    class="alert alert-info"
                  >
                    <LineMdAlertCircle aria-hidden="true" class="size-10" />
                    <div>
                      <h3 class="font-bold">
                        {m.step3_os_metadata_detected_title()}
                      </h3>
                      <p class="text-sm">
                        {m.step3_os_metadata_detected_description()}
                      </p>
                    </div>
                  </div>
                  <label class="label mt-2">
                    <input
                      name="remove-os-metadata"
                      type="checkbox"
                      class="peer checkbox"
                      bind:checked={removeOSMetadataFiles}
                      onchange={rebuildArchive}
                    />
                    <span class="peer-checked:text-primary">
                      {m.step3_remove_os_metadata_files()}
                    </span>
                  </label>
                </div>
              {/if}
              <p class="text-base-content/70">
                {m.step3_description()}
              </p>
              <!-- svelte-ignore a11y_no_redundant_roles -->
              <a
                role="link"
                aria-disabled={!downloadURL || !!busy}
                href={downloadURL && !busy ? downloadURL : undefined}
                download={downloadURL && !busy ? newFilename : undefined}
                class="group btn btn-lg h-auto btn-primary w-full grid grid-cols-[auto_1fr] justify-items-start gap-3 px-3 py-2 not-aria-disabled:data-[downloaded=false]:anim-shine"
                data-downloaded={downloaded}
                data-animate={!!downloadURL && !downloaded}
                onclick={handleDownload}
              >
                {#if busy === "rebuilding"}
                  <LineMdLoadingLoop
                    aria-hidden="true"
                    class="size-10 motion-reduce:hidden"
                  />
                  <span
                    aria-hidden="true"
                    class="size-10 icon-[mdi--hourglass] not-motion-reduce:hidden"
                  ></span>
                  <span>{m.step3_rebuilding()}</span>
                {:else}
                  <LineMdDownloadLoop
                    aria-hidden="true"
                    class="size-10 motion-reduce:hidden group-data-[animate=false]:hidden"
                  />
                  <LineMdDownload
                    aria-hidden="true"
                    class="size-10 not-motion-reduce:group-data-[animate=true]:hidden"
                  />
                  <span>{m.step3_download_button()}</span>
                {/if}
              </a>

              {#if downloaded}
                <a
                  target="_blank"
                  rel="noopener noreferrer"
                  href="https://x.com/intent/tweet?text={encodeURIComponent(
                    m.share_text_after_download({
                      url: location.href.replace(/[?#].*/, ''),
                    }),
                  )}"
                  class="link link-hover text-sm"
                >
                  {m.share_on_x()}
                </a>
              {/if}
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="[view-transition-name:app-footer]">
      <p class="mt-8 text-center text-sm text-base-content/50">
        {m.footer_privacy_text()}
        <a href="/privacy" hreflang="en" class="link link-hover"
          >{m.footer_privacy_link()}</a
        >
      </p>
    </div>
  </div>
</div>
