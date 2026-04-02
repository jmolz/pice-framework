import type {
  ProviderCapabilities,
  InitializeParams,
  InitializeResult,
} from '@pice/provider-protocol';
import { PROVIDER_NOT_INITIALIZED } from '@pice/provider-protocol';
import { StdioTransport } from './transport.js';

/**
 * Abstract base class for PICE providers.
 *
 * Handles the initialize/shutdown lifecycle and capability declaration.
 * Subclasses implement `registerHandlers()` to add provider-specific methods.
 *
 * @typeParam TConfig - The shape of the provider's config object.
 *   Defaults to `unknown` for providers that don't need typed config.
 */
export abstract class BaseProvider<TConfig = unknown> {
  protected transport: StdioTransport;
  protected initialized = false;
  protected config: TConfig | null = null;
  private providerVersion: string;

  constructor(version: string) {
    this.providerVersion = version;
    this.transport = new StdioTransport();
    this.registerCoreHandlers();
    this.registerHandlers(this.transport);
  }

  abstract getCapabilities(): ProviderCapabilities;

  protected abstract registerHandlers(transport: StdioTransport): void;

  async start(): Promise<void> {
    await this.transport.start();
  }

  private registerCoreHandlers(): void {
    this.transport.registerMethod('initialize', async (params: unknown) => {
      const initParams = params as InitializeParams | undefined;
      this.config = (initParams?.config ?? null) as TConfig | null;
      this.initialized = true;

      const result: InitializeResult = {
        capabilities: this.getCapabilities(),
        version: this.providerVersion,
      };
      return result;
    });

    this.transport.registerMethod('shutdown', async () => {
      this.initialized = false;
      await this.onShutdown();
      // The transport writes the response synchronously before this resolves.
      // Schedule exit on next tick to ensure the write is flushed through the pipe.
      setImmediate(() => process.exit(0));
      return null;
    });

    this.transport.registerMethod('capabilities', async () => {
      return this.getCapabilities();
    });
  }

  protected requireInitialized(): void {
    if (!this.initialized) {
      throw Object.assign(new Error('provider not initialized'), {
        code: PROVIDER_NOT_INITIALIZED,
      });
    }
  }

  protected async onShutdown(): Promise<void> {
    // Override in subclasses for cleanup
  }
}
