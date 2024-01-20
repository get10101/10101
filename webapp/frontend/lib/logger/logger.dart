import 'package:get_10101/logger/simple_utc_printer.dart';
import 'package:logger/logger.dart';

// Getter to access the logger instance
Logger get logger => AppLogger.instance;

class AppLogger {
  static late final Logger instance;
}

void buildLogger(bool isLogLevelTrace) {
  final logger = Logger(
      output: ConsoleOutput(),
      filter: ProductionFilter(),
      level: isLogLevelTrace ? Level.trace : Level.debug,
      printer: SimpleUTCPrinter(
          // Colorful log messages
          colors: true,
          // Should each log print contain a timestamp
          printTime: true));

  AppLogger.instance = logger;
}
