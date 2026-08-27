type AmpEvent = { thread?: { id?: string }; status?: string }
type AmpContext = { thread?: { id?: string } }
type Handler = (event: AmpEvent, context: AmpContext) => Promise<void>

const pluginPath = process.argv[2]
if (!pluginPath) throw new Error('Amp plugin path is required')

const handlers = new Map<string, Handler>()
const amp = {
	on(name: string, handler: Handler) {
		handlers.set(name, handler)
	},
}
const pluginModule = await import(pluginPath)
pluginModule.default(amp)

for (const name of ['session.start', 'agent.start', 'agent.end']) {
	if (!handlers.has(name)) throw new Error(`Amp plugin omitted ${name}`)
}

const thread = { id: 'T-ZenttyAmpE2E' }
const context = { thread }
await handlers.get('session.start')!({ thread }, context)
await Bun.sleep(150)
await handlers.get('agent.start')!({ thread }, context)
await Bun.sleep(150)
await handlers.get('agent.end')!({ thread, status: 'stopped' }, context)
await Bun.sleep(150)
await handlers.get('agent.start')!({ thread }, context)
await Bun.sleep(150)
await handlers.get('agent.end')!({ thread, status: 'done' }, context)
