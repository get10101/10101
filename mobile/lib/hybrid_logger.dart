import 'dart:convert';
import 'dart:io';

import 'package:logger/logger.dart';
import 'package:path_provider/path_provider.dart';

/// Writes the log output to a file.
class HybridOutput extends LogOutput {
  final bool overrideExisting;
  final Encoding encoding;
  IOSink? _sink;

  HybridOutput({
    this.overrideExisting = false,
    this.encoding = utf8,
  });

  @override
  Future<void> init() async {
    File logFile = await logFilePath();

    _sink = logFile.openWrite(
      mode: overrideExisting ? FileMode.writeOnly : FileMode.writeOnlyAppend,
      encoding: encoding,
    );
    output(OutputEvent(LogEvent(Level.info, "Test"),
        ["HybridOutputLogger initialized. Will print into console and '${logFile.path}'"]));
  }

  static Future<File> logFilePath() async {
    final dataDir = await getApplicationSupportDirectory();
    final logFile = File('${dataDir.path}/app_log.log');
    return logFile;
  }

  @override
  void output(OutputEvent event) {
    _sink?.writeAll(event.lines, '\n');
    _sink?.writeln();
    // ignore: avoid_print
    event.lines.forEach(print);
  }

  @override
  Future<void> destroy() async {
    await _sink?.flush();
    await _sink?.close();
  }
}
