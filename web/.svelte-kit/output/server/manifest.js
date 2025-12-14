export const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set([]),
	mimeTypes: {},
	_: {
		client: {start:"_app/immutable/entry/start.Cy2j5BjO.js",app:"_app/immutable/entry/app.DF_wb3M6.js",imports:["_app/immutable/entry/start.Cy2j5BjO.js","_app/immutable/chunks/zl-j2mjk.js","_app/immutable/chunks/BzTS1Ps0.js","_app/immutable/chunks/Dp47MLr2.js","_app/immutable/entry/app.DF_wb3M6.js","_app/immutable/chunks/BzTS1Ps0.js","_app/immutable/chunks/Crjah9wm.js","_app/immutable/chunks/D_5r9Cyc.js","_app/immutable/chunks/Dp47MLr2.js","_app/immutable/chunks/bcPC_YCb.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:false},
		nodes: [
			__memo(() => import('./nodes/0.js')),
			__memo(() => import('./nodes/1.js')),
			__memo(() => import('./nodes/2.js'))
		],
		remotes: {
			
		},
		routes: [
			{
				id: "/",
				pattern: /^\/$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 2 },
				endpoint: null
			}
		],
		prerendered_routes: new Set([]),
		matchers: async () => {
			
			return {  };
		},
		server_assets: {}
	}
}
})();
