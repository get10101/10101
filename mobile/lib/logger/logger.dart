import 'package:flutter/foundation.dart';
import 'package:get_10101/logger/hybrid_logger.dart';
import 'package:get_10101/logger/simple_utc_printer.dart';
import 'package:logger/logger.dart';

Logger get logger => AppLogger.instance;

class AppLogger {
  static final Logger _logger = Logger(
      output: HybridOutput(),
      filter: ProductionFilter(),
      // in Debug build we want to log everything on trace in production build we log on debug
      level: kDebugMode ? Level.trace : Level.debug,
      printer: SimpleUTCPrinter(
          // Colorful log messages
          colors: false,
          // Should each log print contain a timestamp
          printTime: true));

  // Getter to access the logger instance
  static final instance = _logger;
}
