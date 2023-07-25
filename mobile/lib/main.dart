import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/init_service.dart';
import 'package:get_10101/common/service_providers.dart';
import 'package:get_10101/common/ten_ten_one_app.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:provider/provider.dart';

void main() {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  setupFlutterLogs();

  const ChannelInfoService channelInfoService = ChannelInfoService();

  final providers = createServiceProviders(
      TradeValuesService(),
      channelInfoService,
      OrderService(),
      PositionService(),
      const WalletService(),
      const CandlestickService(),
      InitService());

  runApp(MultiProvider(providers: providers, child: const TenTenOneApp()));
}

void setupFlutterLogs() {
  final config = FLog.getDefaultConfigurations();
  config.activeLogLevel = LogLevel.TRACE;
  config.formatType = FormatType.FORMAT_CUSTOM;
  config.timestampFormat = 'yyyy-MM-dd HH:mm:ss.SSS';
  config.fieldOrderFormatCustom = [
    FieldName.TIMESTAMP,
    FieldName.LOG_LEVEL,
    FieldName.TEXT,
    FieldName.STACKTRACE
  ];
  config.customClosingDivider = "";
  config.customOpeningDivider = "| ";

  FLog.applyConfigurations(config);
}
