import 'package:candlesticks/candlesticks.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
@GenerateNiceMocks([MockSpec<ChannelInfoService>()])
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/tentenone_config_change_notifier.dart';
@GenerateNiceMocks([MockSpec<DlcChannelService>()])
import 'package:get_10101/common/dlc_channel_service.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/domain/dlc_channel.dart';
import 'package:get_10101/common/domain/model.dart';
@GenerateNiceMocks([MockSpec<CandlestickService>()])
import 'package:get_10101/features/trade/application/candlestick_service.dart';
@GenerateNiceMocks([MockSpec<OrderService>()])
import 'package:get_10101/features/trade/application/order_service.dart';
@GenerateNiceMocks([MockSpec<PositionService>()])
import 'package:get_10101/features/trade/application/position_service.dart';
@GenerateNiceMocks([MockSpec<TradeValuesService>()])
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
@GenerateNiceMocks([MockSpec<WalletService>()])
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/wallet_balances.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/constants.dart';
import 'package:go_router/go_router.dart';
import 'package:mockito/annotations.dart';
import 'package:mockito/mockito.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

import 'trade_test.mocks.dart';

final GoRouter _router = GoRouter(
  initialLocation: TradeScreen.route,
  routes: [
    GoRoute(
        path: TradeScreen.route,
        builder: (BuildContext context, GoRouterState state) {
          return const TradeScreen();
        }),
  ],
);

class TestWrapperWithTradeTheme extends StatelessWidget {
  final Widget child;

  const TestWrapperWithTradeTheme({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      // TODO: We could consider using the Navigator instead of GoRouter to close the bottom sheet again
      // Need GoRouter otherwise closing the bottom sheet after confirmation fails
      routerConfig: _router,
      theme: ThemeData(
        primarySwatch: Colors.blue,
        extensions: const <ThemeExtension<dynamic>>[
          // Need the trade theme otherwise the trade widgets that rely on it can't find it on the context and fail to render
          TradeTheme(),
        ],
      ),
    );
  }
}

void main() {
  buildTestLogger(true);

  MockPositionService positionService = MockPositionService();
  MockTradeValuesService tradeValueService = MockTradeValuesService();
  MockChannelInfoService channelConstraintsService = MockChannelInfoService();
  MockWalletService walletService = MockWalletService();
  MockCandlestickService candlestickService = MockCandlestickService();
  MockDlcChannelService dlcChannelService = MockDlcChannelService();
  MockOrderService orderService = MockOrderService();

  testWidgets('Given trade screen when completing first buy flow then market order is submitted',
      (tester) async {
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
        .thenReturn(Usd(1));
    when(tradeValueService.getExpiryTimestamp()).thenReturn(DateTime.now());
    when(tradeValueService.orderMatchingFee(
            quantity: anyNamed('quantity'), price: anyNamed('price')))
        .thenReturn(Amount(42));
    when(tradeValueService.calculateMaxQuantity(
            price: anyNamed('price'), leverage: anyNamed('leverage')))
        .thenReturn(Usd(2500));

    when(dlcChannelService.getEstimatedChannelFeeReserve()).thenReturn((Amount(500)));

    when(dlcChannelService.getEstimatedFundingTxFee()).thenReturn((Amount(300)));

    when(channelConstraintsService.getTradeConstraints()).thenAnswer((_) =>
        const bridge.TradeConstraints(
            maxLocalMarginSats: 20000000000,
            maxCounterpartyMarginSats: 200000000000,
            coordinatorLeverage: 2,
            minQuantity: 1,
            isChannelBalance: true,
            minMargin: 1,
            maintenanceMarginRate: 0.1,
            orderMatchingFeeRate: 0.003));

    when(candlestickService.fetchCandles(1000)).thenAnswer((_) async {
      return getDummyCandles(1000);
    });
    when(candlestickService.fetchCandles(1)).thenAnswer((_) async {
      return getDummyCandles(1);
    });

    CandlestickChangeNotifier candlestickChangeNotifier =
        CandlestickChangeNotifier(candlestickService);
    candlestickChangeNotifier.initialize();

    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);

    WalletChangeNotifier walletChangeNotifier = WalletChangeNotifier(walletService);

    PositionChangeNotifier positionChangeNotifier = PositionChangeNotifier(positionService);

    TenTenOneConfigChangeNotifier lspChangeNotifier =
        TenTenOneConfigChangeNotifier(channelConstraintsService);

    DlcChannelChangeNotifier dlcChannelChangeNotifier = DlcChannelChangeNotifier(dlcChannelService);

    final tradeValuesChangeNotifier = TradeValuesChangeNotifier(tradeValueService);

    const askPrice = 30000.0;
    const bidPrice = 30000.0;

    // We have to have current price, otherwise we can't take order
    positionChangeNotifier.askPrice = askPrice;
    positionChangeNotifier.bidPrice = bidPrice;
    tradeValuesChangeNotifier.updatePrice(askPrice, Direction.short);
    tradeValuesChangeNotifier.updatePrice(bidPrice, Direction.long);

    await tester.pumpWidget(MultiProvider(providers: [
      ChangeNotifierProvider(create: (context) => tradeValuesChangeNotifier),
      ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
      ChangeNotifierProvider(create: (context) => OrderChangeNotifier(orderService)),
      ChangeNotifierProvider(create: (context) => positionChangeNotifier),
      ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
      ChangeNotifierProvider(create: (context) => walletChangeNotifier),
      ChangeNotifierProvider(create: (context) => candlestickChangeNotifier),
      ChangeNotifierProvider(create: (context) => lspChangeNotifier),
      ChangeNotifierProvider(create: (context) => dlcChannelChangeNotifier),
    ], child: const TestWrapperWithTradeTheme(child: TradeScreen())));

    // We have to pretend that we have a balance, because otherwise the trade bottom sheet validation will not allow us to go to the confirmation screen
    walletChangeNotifier.update(WalletInfo(
        balances: WalletBalances(onChain: Amount(251000), offChain: Amount(100000)), history: []));

    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenButtonBuy), findsOneWidget);

    // Open bottom sheet
    await tester.tap(find.byKey(tradeScreenButtonBuy));
    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenBottomSheetButtonBuy), findsOneWidget);

    // click buy button in bottom sheet
    await tester.tap(find.byKey(tradeScreenBottomSheetButtonBuy));
    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenBottomSheetChannelConfigurationConfirmButton), findsOneWidget);
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.byKey(tradeScreenBottomSheetChannelConfigurationConfirmButton));

    // click confirm button to go to confirmation screen
    await tester.tap(find.byKey(tradeScreenBottomSheetChannelConfigurationConfirmButton));
    await tester.pumpAndSettle();

    // TODO: Use `find.byKey(tradeScreenBottomSheetConfirmationConfigureChannelSlider)`.
    // For some reason the specific widget cannot be found.
    expect(find.byType(ConfirmationSlider), findsOneWidget);

    // Drag to confirm

    // TODO: This is not optimal because if we re-style the component this test will likely break.
    final Offset sliderLocation = tester.getBottomLeft(find.byType(ConfirmationSlider));
    await tester.timedDragFrom(
        sliderLocation + const Offset(10, -15), const Offset(280, 0), const Duration(seconds: 2),
        pointer: 7);

    verify(orderService.submitChannelOpeningMarketOrder(any, any, any, any, any, any, any))
        .called(1);
  });

  testWidgets('Trade with open channel', (tester) async {
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
        .thenReturn(Usd(1));
    when(tradeValueService.getExpiryTimestamp()).thenReturn(DateTime.now());
    when(tradeValueService.orderMatchingFee(
            quantity: anyNamed('quantity'), price: anyNamed('price')))
        .thenReturn(Amount(42));
    when(tradeValueService.calculateMaxQuantity(
            price: anyNamed('price'), leverage: anyNamed('leverage')))
        .thenReturn(Usd(2500));

    when(channelConstraintsService.getTradeConstraints()).thenAnswer((_) =>
        const bridge.TradeConstraints(
            maxLocalMarginSats: 20000000000,
            maxCounterpartyMarginSats: 200000000000,
            coordinatorLeverage: 2,
            minQuantity: 1,
            isChannelBalance: true,
            minMargin: 1,
            maintenanceMarginRate: 0.1,
            orderMatchingFeeRate: 0.003));

    when(dlcChannelService.getEstimatedChannelFeeReserve()).thenReturn((Amount(500)));

    when(dlcChannelService.getEstimatedFundingTxFee()).thenReturn((Amount(300)));

    when(candlestickService.fetchCandles(1000)).thenAnswer((_) async {
      return getDummyCandles(1000);
    });
    when(candlestickService.fetchCandles(1)).thenAnswer((_) async {
      return getDummyCandles(1);
    });

    when(dlcChannelService.getDlcChannels()).thenAnswer((_) async {
      return List.filled(1, DlcChannel(id: "foo", state: ChannelState.signed));
    });

    CandlestickChangeNotifier candlestickChangeNotifier =
        CandlestickChangeNotifier(candlestickService);
    candlestickChangeNotifier.initialize();

    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);

    WalletChangeNotifier walletChangeNotifier = WalletChangeNotifier(walletService);

    PositionChangeNotifier positionChangeNotifier = PositionChangeNotifier(positionService);

    TenTenOneConfigChangeNotifier lspChangeNotifier =
        TenTenOneConfigChangeNotifier(channelConstraintsService);

    DlcChannelChangeNotifier dlcChannelChangeNotifier = DlcChannelChangeNotifier(dlcChannelService);
    dlcChannelChangeNotifier.initialize();

    final tradeValuesChangeNotifier = TradeValuesChangeNotifier(tradeValueService);

    const askPrice = 30000.0;
    const bidPrice = 30000.0;

    // We have to have current price, otherwise we can't take order
    positionChangeNotifier.askPrice = askPrice;
    positionChangeNotifier.bidPrice = bidPrice;
    tradeValuesChangeNotifier.updatePrice(askPrice, Direction.short);
    tradeValuesChangeNotifier.updatePrice(bidPrice, Direction.long);

    await tester.pumpWidget(MultiProvider(providers: [
      ChangeNotifierProvider(create: (context) => tradeValuesChangeNotifier),
      ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
      ChangeNotifierProvider(create: (context) => OrderChangeNotifier(orderService)),
      ChangeNotifierProvider(create: (context) => positionChangeNotifier),
      ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
      ChangeNotifierProvider(create: (context) => walletChangeNotifier),
      ChangeNotifierProvider(create: (context) => candlestickChangeNotifier),
      ChangeNotifierProvider(create: (context) => lspChangeNotifier),
      ChangeNotifierProvider(create: (context) => dlcChannelChangeNotifier),
    ], child: const TestWrapperWithTradeTheme(child: TradeScreen())));

    // We have to pretend that we have a balance, because otherwise the trade bottom sheet validation will not allow us to go to the confirmation screen
    walletChangeNotifier.update(WalletInfo(
        balances: WalletBalances(onChain: Amount(0), offChain: Amount(10000)), history: []));

    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenButtonBuy), findsOneWidget);

    // Open bottom sheet
    await tester.tap(find.byKey(tradeScreenButtonBuy));
    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenBottomSheetButtonBuy), findsOneWidget);

    // click buy button in bottom sheet
    await tester.tap(find.byKey(tradeScreenBottomSheetButtonBuy));
    await tester.pumpAndSettle();

    expect(find.byKey(tradeScreenBottomSheetConfirmationSliderButtonBuy), findsOneWidget);

    await tester.timedDrag(find.byKey(tradeScreenBottomSheetConfirmationSliderButtonBuy),
        const Offset(275, 0), const Duration(seconds: 2),
        pointer: 7);

    verify(orderService.submitMarketOrder(any, any, any, any, any)).called(1);
  });
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
