/** Small synthetic stored ZIPs. External feeder archives never enter tests. */
export function storedZip(files: Record<string, string | Buffer>): Buffer {
	const locals: Buffer[] = [],
		central: Buffer[] = [];
	let offset = 0;
	for (const [path, value] of Object.entries(files)) {
		const name = Buffer.from(path),
			bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
		let crc = 0xffffffff;
		for (const byte of bytes) {
			crc ^= byte;
			for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
		}
		crc = (crc ^ 0xffffffff) >>> 0;
		const local = Buffer.alloc(30);
		local.writeUInt32LE(0x04034b50);
		local.writeUInt16LE(20, 4);
		local.writeUInt16LE(0x800, 6);
		local.writeUInt32LE(crc, 14);
		local.writeUInt32LE(bytes.length, 18);
		local.writeUInt32LE(bytes.length, 22);
		local.writeUInt16LE(name.length, 26);
		const record = Buffer.alloc(46);
		record.writeUInt32LE(0x02014b50);
		record.writeUInt16LE(20, 4);
		record.writeUInt16LE(20, 6);
		record.writeUInt16LE(0x800, 8);
		record.writeUInt32LE(crc, 16);
		record.writeUInt32LE(bytes.length, 20);
		record.writeUInt32LE(bytes.length, 24);
		record.writeUInt16LE(name.length, 28);
		record.writeUInt32LE(offset, 42);
		locals.push(local, name, bytes);
		central.push(record, name);
		offset += local.length + name.length + bytes.length;
	}
	const directory = Buffer.concat(central),
		end = Buffer.alloc(22);
	end.writeUInt32LE(0x06054b50);
	end.writeUInt16LE(central.length / 2, 8);
	end.writeUInt16LE(central.length / 2, 10);
	end.writeUInt32LE(directory.length, 12);
	end.writeUInt32LE(offset, 16);
	return Buffer.concat([...locals, directory, end]);
}

export const day =
	'New Circuit.same basekv=12.47 bus1=source\nRedirect ../shared/line.inc\nNew Load.day bus1=load phases=3 conn=wye kv=12.47 kw=30 kvar=6\n';
export const night = day.replace('Load.day', 'Load.night').replace('kw=30', 'kw=15');
export const line =
	'New Line.l bus1=source bus2=load phases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.4 length=1\n';
export const comparison =
	"mpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [1 3 0 0 0 0 1 1 0 110 1 1.1 0.9];\nmpc.gen = [1 0 0 100 -100 1 100 1 100 0];\nmpc.branch = [];\n";
