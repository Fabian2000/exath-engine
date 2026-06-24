import 'package:exath_engine/exath_engine.dart';
import 'package:flutter/material.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await ensureInitialized(); // loads the WASM module on web; no-op elsewhere
  runApp(const ExathApp());
}

class ExathApp extends StatelessWidget {
  const ExathApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'exath example',
      home: const HomePage(),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});
  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  final _lines = <String>[];

  @override
  void initState() {
    super.initState();
    final s = ExathSession();
    void run(String expr) {
      try {
        _lines.add('$expr  =  ${s.evalLine(expr)}');
      } catch (e) {
        _lines.add('$expr  =>  $e');
      }
    }

    run('2^10 + sqrt(9)');
    run('sqrt(-4)');
    run('diff(sin(x^2), x)');
    run('factor(x^2 - 1, x)');
    run('solve(x^2 - 4, x)');
    run('integral(x^2, x)');
    run('det([[1,2],[3,4]])');
    run('convert(5, km, m)');
    s.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('exath')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          for (final l in _lines)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Text(l, style: const TextStyle(fontFamily: 'monospace')),
            ),
        ],
      ),
    );
  }
}
