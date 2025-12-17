<script lang="ts">
  import init, {
    ZipProcessor,
    type InspectedArchive,
    type InspectConfig,
    type ZipWarning,
    type CompatibilityLevel,
    type FieldSelectionStrategy,
    type EncodingSelectionStrategy,
  } from "bakezip";
  import LineMdCloseCircle from "../icons/LineMdCloseCircle.svelte";
  import LineMdAlert from "../icons/LineMdAlert.svelte";
  import LineMdConfirmCircle from "../icons/LineMdConfirmCircle.svelte";
  import LineMdFolderZip from "../icons/LineMdFolderZip.svelte";
  import LineMdDownloadLoop from "../icons/LineMdDownloadLoop.svelte";
  import LineMdDownload from "../icons/LineMdDownload.svelte";
  import LineMdLoadingLoop from "../icons/LineMdLoadingLoop.svelte";
  import { createI18n } from "../lib/i18n";

  const { locale }: { locale: string } = $props();
  const m = $derived.by(() => createI18n(locale));

  let selectedFile = $state<File | null>(null);
  let inspectedArchive = $state<InspectedArchive | null>(null);
  let processor = $state<ZipProcessor | null>(null);
  let warnings = $state<readonly ZipWarning[]>([]);
  let compatibility = $state<CompatibilityLevel | null>(null);
  let processing = $state(false);
  let error = $state("");

  // Options
  let encoding = $state("__PreferOverallDetected");
  let fieldSelection = $state<FieldSelectionStrategy>(
    "CdhUnicodeThenLfhUnicodeThenCdh",
  );
  let forceProceedToStep2 = $state(false);
  let forceProceedToStep3 = $state(false);

  // Step visibility control
  let step1Complete = $state(false);
  let step2Expanded = $state(false);
  let showStep3 = $state(false);

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

      case "Broken":
        return {
          level: "broken",
          message: "compatibility_broken",
        } as const;

      default:
        return {
          level: "other",
          message: compatibility
            ? "compatibility_other"
            : "compatibility_unknown",
        } as const;
    }
  });

  const decodeErrorCount = $derived.by(() => {
    if (!inspectedArchive) {
      return 0;
    }

    return inspectedArchive.entries.reduce((count, entry) => {
      return entry.filename.decoded?.has_errors !== false ? count + 1 : count;
    }, 0);
  });

  const shouldStopAtStep1 = $derived.by(
    () => compatibilityCategory.level === "ok",
  );

  const shouldStopAtStep2 = $derived.by(() => decodeErrorCount > 0);

  const formatBytes = $derived.by(() => {
    const K = 1024;

    const sizes = [
      m["bytes_unit_b"](),
      m["bytes_unit_kb"](),
      m["bytes_unit_mb"](),
      m["bytes_unit_gb"](),
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

  async function handleFileSelect(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];

    if (!file) return;

    selectedFile = file;
    error = "";

    // Automatically process the ZIP file
    await processZip();
  }

  async function processZip() {
    if (!selectedFile) {
      error = m["step1_error_no_file"]();
      return;
    }

    processing = true;
    error = "";
    step1Complete = false;
    step2Expanded = false;
    showStep3 = false;
    compatibility = null;
    forceProceedToStep2 = false;
    forceProceedToStep3 = false;

    try {
      await init();

      const ts = performance.now();

      // Parse the ZIP file
      processor = await ZipProcessor.parse(selectedFile);

      const elapsed = performance.now() - ts;
      console.info(`Parsed ${selectedFile.name} in ${elapsed} ms`);

      // Ensure minimum loading time for UX
      if (elapsed < 300) {
        await new Promise((resolve) =>
          setTimeout(resolve, Math.floor(300 - elapsed)),
        );
      }

      // Get warnings and compatibility
      warnings = processor.get_warnings();
      compatibility = processor.compatibility;

      step1Complete = true;

      // Inspect with configuration
      const ts2 = performance.now();
      await runInspection();
      const elapsed2 = performance.now() - ts2;
      console.info(`Inspected ${selectedFile.name} in ${elapsed2} ms`);

      // Determine if Step 2 should be expanded
      updateStepVisibility();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      inspectedArchive = null;
      warnings = [];
      compatibility = null;
      processor = null;
    } finally {
      processing = false;
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

  async function runInspection() {
    if (!processor) return;

    const config: InspectConfig = {
      encoding: getEncodingStrategy(encoding),
      field_selection_strategy: fieldSelection,
      ignore_crc32_mismatch: false,
      needs_original_bytes: false,
    };

    forceProceedToStep3 = false;
    inspectedArchive = processor.inspect(config);
  }

  function updateStepVisibility() {
    if (!inspectedArchive) return;

    // Check if encoding detection was successful
    const hasOverallEncoding = !!inspectedArchive.overall_encoding;

    // Step 2: Expand if there's a detection issue, collapse if successful
    step2Expanded = !hasOverallEncoding;

    // Step 3: Show if we have results
    showStep3 = step1Complete;
  }

  // Handle config changes - re-run inspection interactively
  async function handleConfigChange() {
    if (processor) {
      await runInspection();
    }
  }

  function handleDownload() {
    alert(m["step3_download_alert"]());
  }
</script>

<div class="min-h-screen bg-base-300 p-4 md:p-8">
  <div class="mx-auto max-w-6xl">
    <!-- Header -->
    <div class="mb-8 text-center">
      <h1 class="mb-2 text-4xl font-bold text-base-content">
        {m["header_title"]()}
      </h1>
      <p class="text-lg text-base-content/80">
        {m["header_tagline"]()}
      </p>
      <p class="mt-2 text-sm text-base-content/60">
        {m["header_description"]()}
      </p>
    </div>

    <!-- Main Card -->
    <div class="space-y-8">
      <!-- Step 1: ZIP Select and Compatibility Display -->
      <div class="card bg-base-100 shadow-xl">
        <div class="card-body">
          <div class="mb-4 flex items-center gap-3">
            <span
              class="badge badge-primary badge-lg text-center size-8 rounded-full p-0 font-bold before:content-[attr(data-step)]"
              data-step="1"
              aria-label={m["step1_alt"]()}
            ></span>
            <h2 class="card-title text-2xl">{m["step1_title"]()}</h2>
          </div>

          <!-- File Upload Section -->
          <div class="mb-6">
            <p class="mb-3 text-sm text-base-content/70">
              {m["step1_info"]()}
            </p>
            <div
              class="relative flex items-center justify-center rounded-lg border-2 border-dashed border-primary/30 bg-base-200 px-3 py-6 not-motion-reduce:transition cursor-wait has-focus-visible:border-primary has-enabled:cursor-pointer has-enabled:hover:border-primary has-enabled:hover:bg-base-300"
            >
              <input
                type="file"
                accept=".zip"
                disabled={processing}
                class="absolute inset-0 cursor-wait enabled:cursor-pointer opacity-0 appearance-none"
                onchange={handleFileSelect}
              />
              <div class="grid grid-cols-1 place-items-center text-center">
                <LineMdFolderZip class="text-base-content size-14" />
                {#if selectedFile}
                  <div class="text-base-content/70 pt-2">
                    {selectedFile.name}
                  </div>
                  <div class="pt-1 text-sm text-base-content/50">
                    {formatBytes(selectedFile.size)}
                  </div>
                {:else}
                  <p class="text-base-content/70 pt-2">
                    {m["step1_file_placeholder"]()}
                  </p>
                {/if}
              </div>
            </div>
          </div>

          {#if processing}
            <div role="alert" class="alert alert-info" aria-live="polite">
              <LineMdLoadingLoop class="size-10 motion-reduce:hidden" />
              <span
                class="size-10 icon-[mdi--hourglass] not-motion-reduce:hidden"
              ></span>
              <span>{m["step1_processing"]()}</span>
            </div>
          {/if}

          <!-- Error Message -->
          {#if error}
            <div role="alert" aria-live="polite" class="alert alert-error">
              <LineMdCloseCircle class="size-10" />
              <span>{error}</span>
            </div>
          {/if}

          <!-- Warnings -->
          {#if warnings.length > 0}
            <div
              role="status"
              aria-live="polite"
              class="alert alert-soft alert-warning flex-col items-start"
            >
              <div class="flex items-center gap-2">
                <LineMdAlert class="size-10" />
                <h3 class="font-bold">
                  {m["step1_warnings_title"]({ count: warnings.length })}
                </h3>
              </div>
              <ul class="list-disc list-inside text-sm">
                {#each warnings as warning}
                  <li>
                    {#if warning.index !== null && warning.index !== undefined}
                      <span class="[font-feature-settings:tnum] min-w-30"
                        >{m["step1_warning_entry_prefix"]({
                          index: warning.index,
                        })}</span
                      >
                    {/if}
                    {warning.message}
                  </li>
                {/each}
              </ul>
            </div>
          {/if}

          <!-- Compatibility Check Results -->
          {#if step1Complete && compatibility}
            <div
              role="alert"
              aria-live="polite"
              data-compatibility={compatibilityCategory.level}
              class="alert data-[compatibility=ok]:alert-success data-[compatibility=broken]:alert-error data-[compatibility=other]:alert-warning"
            >
              {#if compatibilityCategory.level === "ok"}
                <LineMdConfirmCircle class="size-10" />
              {:else if compatibilityCategory.level === "broken"}
                <LineMdCloseCircle class="size-10" />
              {:else}
                <LineMdAlert class="size-10" />
              {/if}
              <div>
                <h3 class="font-bold">{m["compatibility_title"]()}</h3>
                <p class="text-sm">
                  {m[compatibilityCategory.message]()}
                </p>
              </div>
            </div>
          {/if}
          {#if shouldStopAtStep1}
            <div class="mt-4">
              <label class="label">
                <input
                  name="force-step2"
                  type="checkbox"
                  class="peer checkbox"
                  bind:checked={forceProceedToStep2}
                />
                <span class="peer-checked:text-primary"
                  >{m["step1_process_anyway"]()}</span
                >
              </label>
            </div>
          {/if}
        </div>
      </div>

      <!-- Step 2: Configuration (Optional, if needed) -->
      {#if step1Complete && (!shouldStopAtStep1 || forceProceedToStep2)}
        <div class="card bg-base-100 shadow-xl">
          <div class="card-body">
            <button
              class="flex w-full items-center gap-3 text-left"
              onclick={() => {
                step2Expanded = !step2Expanded;
              }}
            >
              <span
                class="badge badge-primary badge-lg text-center size-8 rounded-full p-0 font-bold before:content-[attr(data-step)]"
                data-step="2"
                aria-label={m["step2_alt"]()}
              ></span>
              <h2 class="card-title flex-1 text-2xl">{m["step2_title"]()}</h2>
              <span
                class="icon-[mdi--chevron-down] size-8 not-motion-reduce:transition-transform data-[expanded=true]:rotate-180"
                data-expanded={step2Expanded}
              ></span>
            </button>

            <div class="group space-y-6 pt-4" data-expanded={step2Expanded}>
              <!-- Encoding Selection -->
              <fieldset class="fieldset group-data-[expanded=false]:hidden">
                <legend class="fieldset-legend text-sm"
                  >{m["step2_encoding_label"]()}</legend
                >
                <select
                  name="encoding-select"
                  class="select w-full"
                  bind:value={encoding}
                  onchange={handleConfigChange}
                >
                  <option value="__PreferOverallDetected"
                    >{m["step2_encoding_auto_overall"]()}</option
                  >
                  <option value="__EntryDetected"
                    >{m["step2_encoding_auto_entry"]()}</option
                  >
                  <option value="UTF-8"
                    >{m["step2_encoding_force_utf8"]()}</option
                  >
                  <option value="Shift_JIS"
                    >{m["step2_encoding_force_shift_jis"]()}</option
                  >
                  <option value="EUC-KR"
                    >{m["step2_encoding_force_euc_kr"]()}</option
                  >
                  <option value="GBK">{m["step2_encoding_force_gbk"]()}</option>
                  <option value="Big5"
                    >{m["step2_encoding_force_big5"]()}</option
                  >
                </select>
                <p class="label">
                  {m["step2_encoding_force_note"]()}
                </p>
              </fieldset>

              <!-- Field Selection -->
              <fieldset class="fieldset group-data-[expanded=false]:hidden">
                <legend class="fieldset-legend text-sm"
                  >{m["step2_field_selection_label"]()}</legend
                >
                <select
                  name="field-select"
                  class="select w-full"
                  bind:value={fieldSelection}
                  onchange={handleConfigChange}
                >
                  <option value="CdhUnicodeThenLfhUnicodeThenCdh"
                    >{m["step2_field_cdh_unicode_lfh_unicode_cdh"]()}</option
                  >
                  <option value="CdhUnicodeThenLfhUnicodeThenLfh"
                    >{m["step2_field_cdh_unicode_lfh_unicode_lfh"]()}</option
                  >
                  <option value="LfhUnicodeThenCdhUnicodeThenCdh"
                    >{m["step2_field_lfh_unicode_cdh_unicode_cdh"]()}</option
                  >
                  <option value="LfhUnicodeThenCdhUnicodeThenLfh"
                    >{m["step2_field_lfh_unicode_cdh_unicode_lfh"]()}</option
                  >
                  <option value="CdhUnicodeThenCdh"
                    >{m["step2_field_cdh_unicode_cdh"]()}</option
                  >
                  <option value="LfhUnicodeThenLfh"
                    >{m["step2_field_lfh_unicode_lfh"]()}</option
                  >
                  <option value="CdhOnly">{m["step2_field_cdh_only"]()}</option>
                  <option value="LfhOnly">{m["step2_field_lfh_only"]()}</option>
                </select>
                <p class="label">
                  {m["step2_field_selection_note"]()}
                </p>
              </fieldset>

              <!-- Decoded Filenames Preview -->
              {#if inspectedArchive}
                <div>
                  <h3
                    class="mb-3 text-lg font-semibold group-data-[expanded=false]:hidden"
                  >
                    {m["step2_decoded_filenames_title"]()}
                  </h3>

                  {#if inspectedArchive.overall_encoding}
                    <div class="alert alert-success py-2">
                      <LineMdConfirmCircle class="size-10" />
                      <div>
                        <h3 class="font-bold">
                          {m["step2_detected_encoding_title"]()}
                        </h3>
                        <p class="text-lg font-bold">
                          {inspectedArchive.overall_encoding}
                        </p>
                      </div>
                    </div>
                  {:else}
                    <div class="alert alert-warning py-2">
                      <LineMdAlert class="size-10" />
                      <div>
                        <h3 class="font-bold">
                          {m["step2_detected_encoding_title"]()}
                        </h3>
                        <p class="text-sm">
                          {m["step2_detected_encoding_none"]()}
                        </p>
                      </div>
                    </div>
                  {/if}

                  <div
                    class="group-data-[expanded=false]:hidden max-h-[calc(100vh-16rem)] mt-4 overflow-auto rounded-lg border border-base-200"
                  >
                    <table class="table table-zebra table-pin-rows w-full">
                      <thead class="bg-base-200">
                        <tr>
                          <th>{m["step2_table_filename"]()}</th>
                          <th>{m["step2_table_detected_encoding"]()}</th>
                          <th>{m["step2_table_field_type"]()}</th>
                          <th class="text-center"
                            >{m["step2_table_utf8_flag"]()}</th
                          >
                        </tr>
                      </thead>
                      <tbody>
                        {#each inspectedArchive.entries as entry}
                          <tr>
                            <td
                              class="min-w-40 truncate data-[error=true]:text-error"
                              data-error={entry.filename.decoded?.has_errors !==
                                false}
                              title={entry.filename.decoded?.string}
                            >
                              {#if entry.filename.decoded}
                                {entry.filename.decoded.string}
                              {:else}
                                <span class="italic">
                                  {m["step2_unable_to_decode"]()}
                                </span>
                              {/if}
                            </td>
                            <td
                              class="w-50 data-[warning=true]:text-warning"
                              data-warning={entry.filename.decoded
                                ?.encoding_used !==
                                entry.filename.detected_encoding &&
                                entry.filename.detected_encoding !== "ASCII"}
                            >
                              {entry.filename.decoded?.encoding_used ??
                                m["step2_table_encoding_na"]()}
                              {#if entry.filename.detected_encoding && entry.filename.detected_encoding !== entry.filename.decoded?.encoding_used}
                                ({entry.filename.detected_encoding})
                              {/if}
                            </td>
                            <td class="w-40 text-base-content/70">
                              {entry.filename.kind}
                            </td>
                            <td class="w-30 text-center">
                              <span class="grid place-items-center">
                                {#if entry.filename.utf8_flag}
                                  <span class="icon-[mdi--check] text-success"
                                  ></span>
                                {:else}
                                  <span
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
                        {m["step2_stats_total_files"]()}
                      </div>
                      <div class="stat-value">
                        {inspectedArchive.entries.length}
                      </div>
                    </div>
                    {#if decodeErrorCount > 0}
                      <div class="stat">
                        <div class="stat-title">
                          {m["step2_stats_decoding_errors"]()}
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

            {#if shouldStopAtStep2}
              <div class="mt-4">
                <label class="label">
                  <input
                    name="force-step3"
                    type="checkbox"
                    class="peer checkbox"
                    bind:checked={forceProceedToStep3}
                  />
                  <span class="peer-checked:text-primary"
                    >{m["step2_ignore_errors"]()}</span
                  >
                </label>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Step 3: Convert and Download -->
      {#if showStep3 && (!shouldStopAtStep1 || forceProceedToStep2) && (!shouldStopAtStep2 || forceProceedToStep3)}
        <div class="card bg-base-100 shadow-xl">
          <div class="card-body">
            <div class="mb-4 flex items-center gap-3">
              <span
                class="badge badge-primary badge-lg text-center size-8 rounded-full p-0 font-bold before:content-[attr(data-step)]"
                data-step="3"
                aria-label={m["step3_alt"]()}
              ></span>
              <h2 class="card-title text-2xl">{m["step3_title"]()}</h2>
            </div>

            <div class="space-y-4">
              <p class="text-base-content/70">
                {m["step3_description"]()}
              </p>
              <button
                onclick={handleDownload}
                class="btn btn-lg h-auto btn-primary w-full grid grid-cols-[auto_1fr] justify-items-start gap-3 px-3 py-2"
              >
                <LineMdDownloadLoop class="size-10 motion-reduce:hidden" />
                <LineMdDownload class="size-10 not-motion-reduce:hidden" />
                <span>{m["step3_download_button"]()}</span>
              </button>
            </div>
          </div>
        </div>
      {/if}
    </div>
    <div>
      <p class="mt-8 text-center text-sm text-base-content/50">
        {m["footer_privacy_text"]()}
        <a href="/privacy-policy" class="link link-hover"
          >{m["footer_privacy_link"]()}</a
        >
      </p>
    </div>
  </div>
</div>
