import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart' as foundation;
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'ffi.dart';

void main() {
  final config = FLog.getDefaultConfigurations();
  config.activeLogLevel = LogLevel.DEBUG;

  FLog.applyConfigurations(config);

  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '10101',
      theme: ThemeData(
        primarySwatch: Colors.blue,
      ),
      home: const TenTenOneApp(title: '10101'),
    );
  }
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key, required this.title}) : super(key: key);

  final String title;

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {

  bool ready = false;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.title),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: const <Widget>[
            Text("Welcome to 10101"),
          ],
        ),
      ),
    );
  }

  Future<void> init() async {
    try {
      await setupRustLogging();

      setState(() {
        FLog.info(text: "10101 is ready!");
        ready = true;
      });
    } on FfiException catch (error) {
      FLog.error(text: "Failed to initialise: Error: ${error.message}", exception: error);
    } catch (error) {
      FLog.error(text: "Failed to initialise: Unknown error");
    }
  }

  Future<void> setupRustLogging() async {
    api.initLogging().listen((event) {
      // Only log to Dart file in release mode - in debug mode it's easier to
      // use stdout
      if (foundation.kReleaseMode) {
        FLog.logThis(text: '${event.target}: ${event.msg}', type: LogLevel.DEBUG);
      }
    });
  }


}
