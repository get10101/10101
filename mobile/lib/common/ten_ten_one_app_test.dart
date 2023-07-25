import 'package:candlesticks/candlesticks.dart';
import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/service_providers.dart';
import 'package:get_10101/common/ten_ten_one_app.dart';
import 'package:mockito/annotations.dart';
import 'package:mockito/mockito.dart';
import 'package:provider/provider.dart';

@GenerateNiceMocks([MockSpec<TradeValuesService>()])
import 'package:get_10101/features/trade/application/trade_values_service.dart';

@GenerateNiceMocks([MockSpec<ChannelInfoService>()])
import 'package:get_10101/common/application/channel_info_service.dart';

@GenerateNiceMocks([MockSpec<OrderService>()])
import 'package:get_10101/features/trade/application/order_service.dart';

@GenerateNiceMocks([MockSpec<PositionService>()])
import 'package:get_10101/features/trade/application/position_service.dart';

@GenerateNiceMocks([MockSpec<WalletService>()])
import 'package:get_10101/features/wallet/application/wallet_service.dart';

@GenerateNiceMocks([MockSpec<CandlestickService>()])
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:provider/provider.dart';

@GenerateNiceMocks([MockSpec<InitService>()])
import 'package:get_10101/common/domain/init_service.dart';

import 'ten_ten_one_app_test.mocks.dart';

void main() {
  testWidgets('App starts up correctly without the backend',
      (tester) async {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();

  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  MockOrderService orderService = MockOrderService();
  MockPositionService positionService = MockPositionService();
  MockTradeValuesService tradeValueService = MockTradeValuesService();
  MockChannelInfoService channelConstraintsService = MockChannelInfoService();
  MockWalletService walletService = MockWalletService();
  MockCandlestickService candlestickService = MockCandlestickService();
  MockInitService init = MockInitService();

  setupFlutterLogs();

    // TODO: we could make this more resilient in the underlying components...
    // return dummies otherwise the fields won't be initialized correctly
    when(tradeValueService.calculateMargin(
            price: anyNamed('price'),
            quantity: anyNamed('quantity'),
            leverage: anyNamed('leverage')))
        .thenReturn(Amount(1000));
    when(tradeValueService.calculateLiquidationPrice(
            price: anyNamed('price'),
            leverage: anyNamed('leverage'),
            direction: anyNamed('direction')))
        .thenReturn(10000);
    when(tradeValueService.calculateQuantity(
            price: anyNamed('price'), leverage: anyNamed('leverage'), margin: anyNamed('margin')))
        .thenReturn(0.1);

    // assuming this is an initial funding, no channel exists yet
    when(channelConstraintsService.getChannelInfo()).thenAnswer((_) async {
      return null;
    });

    when(channelConstraintsService.getMaxCapacity()).thenReturn(Amount(20000));

    when(channelConstraintsService.getMinTradeMargin()).thenReturn(Amount(1000));

    when(channelConstraintsService.getInitialReserve()).thenReturn(Amount(1000));

    when(channelConstraintsService.getTradeFeeReserve()).thenReturn(Amount(1666));

    when(channelConstraintsService.getCoordinatorLiquidityMultiplier()).thenReturn(2);

    when(candlestickService.fetchCandles(1000)).thenAnswer((_) async {
      return getDummyCandles(1000);
    });
    when(candlestickService.fetchCandles(1)).thenAnswer((_) async {
      return getDummyCandles(1);
    });

    // We have to have current price, otherwise we can't take order
    // TODO: How to solve this
    // positionChangeNotifier.price = Price(bid: 30000.0, ask: 30000.0);

    final providers = createServiceProviders(tradeValueService, channelConstraintsService, orderService, positionService, walletService, candlestickService, init);

    await tester.pumpWidget(MultiProvider(providers: providers,
   child: const TenTenOneApp()));

    await tester.pump();
  });
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

List<Candle> getDummyCandles(int amount) {
  List<Candle> candles = List.empty(growable: true);
  for (int i = 0; i < amount; i++) {
    candles.add(Candle(
      date: DateTime.now(),
      close: 23.000,
      high: 24.000,
      low: 22.000,
      open: 22.000,
      volume: 23.000,
    ));
  }
  return candles;
}
