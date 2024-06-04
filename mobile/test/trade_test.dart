import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:get_10101/common/amount_text_field.dart';
@GenerateNiceMocks([MockSpec<ChannelInfoService>()])
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/tentenone_config_change_notifier.dart';
@GenerateNiceMocks([MockSpec<DlcChannelService>()])
import 'package:get_10101/common/dlc_channel_service.dart';
@GenerateNiceMocks([MockSpec<TradeService>()])
import 'package:get_10101/features/trade/application/trade_service.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/domain/dlc_channel.dart';
import 'package:get_10101/common/domain/model.dart';
@GenerateNiceMocks([MockSpec<OrderService>()])
import 'package:get_10101/features/trade/application/order_service.dart';
@GenerateNiceMocks([MockSpec<PositionService>()])
import 'package:get_10101/features/trade/application/position_service.dart';
@GenerateNiceMocks([MockSpec<TradeValuesService>()])
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/channel_creation_flow/channel_configuration_screen.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/funding_rate_change_notifier.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_change_notifier.dart';
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

GoRouter buildGoRouterMock(String initialLocation) {
  return GoRouter(
    initialLocation: initialLocation,
    routes: [
      GoRoute(
          path: TradeScreen.route,
          builder: (BuildContext context, GoRouterState state) {
            return const TradeScreen();
          }),
      GoRoute(
          path: ChannelConfigurationScreen.route,
          builder: (BuildContext context, GoRouterState state) {
            return const ChannelConfigurationScreen(
              direction: Direction.long,
            );
          }),
    ],
  );
}

class TestWrapperWithTradeTheme extends StatelessWidget {
  final Widget child;
  final RouterConfig<Object> router;

  const TestWrapperWithTradeTheme({super.key, required this.child, required this.router});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      // TODO: We could consider using the Navigator instead of GoRouter to close the bottom sheet again
      // Need GoRouter otherwise closing the bottom sheet after confirmation fails
      routerConfig: router,
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
  MockDlcChannelService dlcChannelService = MockDlcChannelService();
  MockOrderService orderService = MockOrderService();
  MockTradeService tradeService = MockTradeService();

  testWidgets('Given rates, the trade screen show bid/ask price', (tester) async {
    final tradeValuesChangeNotifier = TradeValuesChangeNotifier(tradeValueService);
    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);
    PositionChangeNotifier positionChangeNotifier = PositionChangeNotifier(positionService);
    TradeChangeNotifier tradeChangeNotifier = TradeChangeNotifier(tradeService);
    FundingRateChangeNotifier fundingRateChangeNotifier = FundingRateChangeNotifier();

    const askPrice = 30001.0;
    const bidPrice = 30000.0;

    positionChangeNotifier.askPrice = askPrice;
    positionChangeNotifier.bidPrice = bidPrice;
    tradeValuesChangeNotifier.updatePrice(askPrice, Direction.short);
    tradeValuesChangeNotifier.updatePrice(bidPrice, Direction.long);

    // We start the trade screen
    await tester.pumpWidget(MultiProvider(
        providers: [
          ChangeNotifierProvider(create: (context) => tradeValuesChangeNotifier),
          ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
          ChangeNotifierProvider(create: (context) => OrderChangeNotifier(orderService)),
          ChangeNotifierProvider(create: (context) => positionChangeNotifier),
          ChangeNotifierProvider(create: (context) => tradeChangeNotifier),
          ChangeNotifierProvider(create: (context) => fundingRateChangeNotifier),
        ],
        child: TestWrapperWithTradeTheme(
          router: buildGoRouterMock(TradeScreen.route),
          child: const TradeScreen(),
        )));
    logger.i("Trade screen started");

    // We check if all the widgets are here which we want to see
    var tradeScreenAskPriceWidget = find.byKey(tradeScreenAskPrice);
    expect(tradeScreenAskPriceWidget, findsOneWidget);
    var assertedPrice = assertPrice(tester, tradeScreenAskPriceWidget, "\$30,001");
    logger.i("Ask price found: $assertedPrice");
    var tradeScreenBidPriceWidget = find.byKey(tradeScreenBidPrice);
    expect(tradeScreenBidPriceWidget, findsOneWidget);
    assertedPrice = assertPrice(tester, tradeScreenBidPriceWidget, "\$30,000");
    logger.i("Bid price found: $assertedPrice");

    // Buy and sell buttons are also here
    expect(find.byKey(tradeScreenButtonBuy), findsOneWidget);
    logger.i("Buy button found");
    expect(find.byKey(tradeScreenButtonSell), findsOneWidget);
    logger.i("Sell button found");

    // The two tabs for positions and orders are also here
    expect(find.byKey(tradeScreenTabsPositions), findsOneWidget);
    logger.i("Positions tab button found");
    expect(find.byKey(tradeScreenTabsOrders), findsOneWidget);
    logger.i("Orders tab button found");
  });

  testWidgets('Given price and balance we see maximum quantity and margin set', (tester) async {
    final tradeValuesChangeNotifier = TradeValuesChangeNotifier(tradeValueService);
    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);
    PositionChangeNotifier positionChangeNotifier = PositionChangeNotifier(positionService);
    TenTenOneConfigChangeNotifier configChangeNotifier =
        TenTenOneConfigChangeNotifier(channelConstraintsService);
    DlcChannelChangeNotifier dlcChannelChangeNotifier = DlcChannelChangeNotifier(dlcChannelService);
    OrderChangeNotifier orderChangeNotifier = OrderChangeNotifier(orderService);
    TradeChangeNotifier tradeChangeNotifier = TradeChangeNotifier(tradeService);
    FundingRateChangeNotifier fundingRateChangeNotifier = FundingRateChangeNotifier();

    const askPrice = 30001.0;
    const bidPrice = 30000.0;

    positionChangeNotifier.askPrice = askPrice;
    positionChangeNotifier.bidPrice = bidPrice;
    tradeValuesChangeNotifier.updatePrice(askPrice, Direction.short);
    tradeValuesChangeNotifier.updatePrice(bidPrice, Direction.long);

    var mockedDefaultMargin = Amount(1000);
    when(tradeValueService.calculateMargin(
            price: anyNamed('price'),
            quantity: anyNamed('quantity'),
            leverage: anyNamed('leverage')))
        .thenReturn(mockedDefaultMargin);
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
    var mockedMaxQuantity = Usd(2500);
    when(tradeValueService.calculateMaxQuantity(
            price: anyNamed('price'),
            leverage: anyNamed('leverage'),
            direction: anyNamed('direction')))
        .thenReturn(mockedMaxQuantity);
    when(dlcChannelService.getEstimatedChannelFeeReserve()).thenReturn(Amount(123));
    when(dlcChannelService.getEstimatedFundingTxFee()).thenReturn(Amount(42));

    when(channelConstraintsService.getTradeConstraints()).thenAnswer((_) =>
        const bridge.TradeConstraints(
            maxLocalBalanceSats: 20000000000,
            maxCounterpartyBalanceSats: 200000000000,
            coordinatorLeverage: 2,
            minQuantity: 1,
            isChannelBalance: true,
            minMargin: 1,
            maintenanceMarginRate: 0.1,
            orderMatchingFeeRate: 0.003));

    // We start the trade screen
    await tester.pumpWidget(MultiProvider(
        providers: [
          ChangeNotifierProvider(create: (context) => tradeValuesChangeNotifier),
          ChangeNotifierProvider(create: (context) => configChangeNotifier),
          ChangeNotifierProvider(create: (context) => dlcChannelChangeNotifier),
          ChangeNotifierProvider(create: (context) => orderChangeNotifier),
          ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
          ChangeNotifierProvider(create: (context) => positionChangeNotifier),
          ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
          ChangeNotifierProvider(create: (context) => tradeChangeNotifier),
          ChangeNotifierProvider(create: (context) => fundingRateChangeNotifier),
        ],
        child: TestWrapperWithTradeTheme(
          router: buildGoRouterMock(TradeScreen.route),
          child: const TradeScreen(),
        )));

    logger.i("Trade screen started");

    // Just check for the buy button to open the bottom sheet
    expect(find.byKey(tradeScreenButtonBuy), findsOneWidget);
    logger.i("Buy button found");

    // Open bottom sheet
    await tester.tap(find.byKey(tradeScreenButtonBuy));
    await tester.pumpAndSettle();
    logger.i("Trade bottom sheet opened");

    // Assert market price
    {
      var marketPriceWidget = find.byKey(tradeButtonSheetMarketPrice);
      expect(marketPriceWidget, findsOneWidget);
      logger.i("Market price field found");

      // Find the Text widget within the marketPriceWidget
      final usdWidgetTextFields = find.descendant(
        of: marketPriceWidget,
        matching: find.byType(Text),
      );

      // Verify the Text widget is found
      expect(usdWidgetTextFields, findsWidgets);

      // Check if the widget contains our market price
      bool containsDesiredString = false;
      usdWidgetTextFields.evaluate().forEach((element) {
        final textWidget = element.widget as Text;
        if (textWidget.data == "30,001") {
          containsDesiredString = true;
        }
      });
      expect(containsDesiredString, isTrue);
      logger.i("Market price found");
    }

    // Find quantity input field and assert this field is set
    {
      var quantityInputFieldWidget = find.byKey(tradeButtonSheetQuantityInput);
      expect(quantityInputFieldWidget, findsOneWidget);
      logger.i("Quantity input field found");
      // Find the input field widget
      final quantityInputField = find.descendant(
        of: quantityInputFieldWidget,
        matching: find.byType(TextFormField),
      );
      expect(quantityInputField, findsOneWidget);

      // Verify the default text in input field
      final textFormField = tester.widget<TextFormField>(quantityInputField);
      expect(textFormField.controller?.text, mockedMaxQuantity.formatted());
      logger.i("Initial quantity field was set to: ${textFormField.controller?.text}");
    }

    // Find margin field and verify it has been set correctly
    {
      verifyMarginFieldValueSet(tester, mockedDefaultMargin);
    }

    // Update the input field and verify that margin has been recomputed
    {
      var quantityInputFieldWidget = find.byKey(tradeButtonSheetQuantityInput);
      expect(quantityInputFieldWidget, findsOneWidget);
      logger.i("Quantity input field widget found");
      // Find the input field widget
      final quantityInputField = find.descendant(
        of: quantityInputFieldWidget,
        matching: find.byType(TextFormField),
      );
      expect(quantityInputField, findsOneWidget);
      logger.i("Quantity input field found");

      // Verify the default text in input field
      final textFormField = tester.widget<TextFormField>(quantityInputField);
      // Enter text into the TextFormField
      await tester.enterText(quantityInputField, '100');
      var inputQuantity = Usd(100);
      expect(textFormField.controller?.text, inputQuantity.formatted());
      logger.i("Updated quantity field was set to: ${textFormField.controller?.text}");

      verify(tradeValueService.calculateMargin(
              price: 30001.0, quantity: inputQuantity, leverage: Leverage(2)))
          .called(greaterThan(1));

      logger.i("Margin has been recalculated");
    }

    // we verify again if we can find the buy button but do not click it
    // our test setup does not support navigating unfortunately
    expect(find.byKey(tradeScreenBottomSheetButtonBuy), findsOneWidget);
    logger.i("Found buy button");
  });

  testWidgets('when funding with internal wallet, then market buy order is created',
      (tester) async {
    // This is to ensure we don't get random overflows. The dimensions are from an iPhone 15
    await tester.binding.setSurfaceSize(const Size(2556, 1179));

    final tradeValuesChangeNotifier = TradeValuesChangeNotifier(tradeValueService);
    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);
    PositionChangeNotifier positionChangeNotifier = PositionChangeNotifier(positionService);
    TenTenOneConfigChangeNotifier configChangeNotifier =
        TenTenOneConfigChangeNotifier(channelConstraintsService);
    DlcChannelChangeNotifier dlcChannelChangeNotifier = DlcChannelChangeNotifier(dlcChannelService);
    OrderChangeNotifier orderChangeNotifier = OrderChangeNotifier(orderService);
    TradeChangeNotifier tradeChangeNotifier = TradeChangeNotifier(tradeService);
    FundingRateChangeNotifier fundingRateChangeNotifier = FundingRateChangeNotifier();

    const askPrice = 30001.0;
    const bidPrice = 30000.0;

    var mockedDefaultMargin = Amount(1000);
    when(tradeValueService.calculateMargin(
            price: anyNamed('price'),
            quantity: anyNamed('quantity'),
            leverage: anyNamed('leverage')))
        .thenReturn(mockedDefaultMargin);
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
    var mockedMaxQuantity = Usd(2500);
    when(tradeValueService.calculateMaxQuantity(
            price: anyNamed('price'),
            leverage: anyNamed('leverage'),
            direction: anyNamed('direction')))
        .thenReturn(mockedMaxQuantity);
    when(dlcChannelService.getEstimatedChannelFeeReserve()).thenReturn(Amount(123));
    when(dlcChannelService.getEstimatedFundingTxFee()).thenReturn(Amount(42));

    positionChangeNotifier.askPrice = askPrice;
    positionChangeNotifier.bidPrice = bidPrice;
    tradeValuesChangeNotifier.maxQuantityLock = false;
    tradeValuesChangeNotifier.updatePrice(askPrice, Direction.short);
    tradeValuesChangeNotifier.updatePrice(bidPrice, Direction.long);

    when(channelConstraintsService.getTradeConstraints()).thenAnswer((_) =>
        const bridge.TradeConstraints(
            maxLocalBalanceSats: 10000000,
            maxCounterpartyBalanceSats: 20000000,
            coordinatorLeverage: 2,
            minQuantity: 1,
            isChannelBalance: true,
            minMargin: 1,
            maintenanceMarginRate: 0.1,
            orderMatchingFeeRate: 0.003));

    // We start the trade screen
    await tester.pumpWidget(MultiProvider(
        providers: [
          ChangeNotifierProvider(create: (context) => tradeValuesChangeNotifier),
          ChangeNotifierProvider(create: (context) => configChangeNotifier),
          ChangeNotifierProvider(create: (context) => dlcChannelChangeNotifier),
          ChangeNotifierProvider(create: (context) => orderChangeNotifier),
          ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
          ChangeNotifierProvider(create: (context) => positionChangeNotifier),
          ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
          ChangeNotifierProvider(create: (context) => tradeChangeNotifier),
          ChangeNotifierProvider(create: (context) => fundingRateChangeNotifier),
        ],
        child: TestWrapperWithTradeTheme(
          router: buildGoRouterMock(ChannelConfigurationScreen.route),
          child: const ChannelConfigurationScreen(direction: Direction.long),
        )));

    logger.i("Channel configuration screen started");

    expect(find.byKey(tradeScreenBottomSheetChannelConfigurationConfirmButton), findsOneWidget);
    logger.i("Confirmation button is present");
    var checkboxFinder =
        find.byKey(tradeScreenBottomSheetChannelConfigurationFundWithWalletCheckBox);
    expect(checkboxFinder, findsOneWidget);
    logger.i("Checkbox is present");

    // Tap the checkbox to check it
    await tester.tap(checkboxFinder);
    await tester.pumpAndSettle();
    logger.i("Checked the checkbox");

    // Verify the checkbox is checked
    expect(tester.widget<Checkbox>(checkboxFinder).value, true);
    logger.i("Verified that it is checked");

    expect(find.byKey(tradeScreenBottomSheetChannelConfigurationConfirmSlider), findsOneWidget);
    logger.i("Confirmation slider is now present");

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
            price: anyNamed('price'),
            leverage: anyNamed('leverage'),
            direction: anyNamed('direction')))
        .thenReturn(Usd(2500));

    when(channelConstraintsService.getTradeConstraints()).thenAnswer((_) =>
        const bridge.TradeConstraints(
            maxLocalBalanceSats: 20000000000,
            maxCounterpartyBalanceSats: 200000000000,
            coordinatorLeverage: 2,
            minQuantity: 1,
            isChannelBalance: true,
            minMargin: 1,
            maintenanceMarginRate: 0.1,
            orderMatchingFeeRate: 0.003));

    when(dlcChannelService.getEstimatedChannelFeeReserve()).thenReturn((Amount(500)));

    when(dlcChannelService.getEstimatedFundingTxFee()).thenReturn((Amount(300)));

    when(dlcChannelService.getDlcChannels()).thenAnswer((_) async {
      return List.filled(1, DlcChannel(id: "foo", state: ChannelState.signed));
    });

    SubmitOrderChangeNotifier submitOrderChangeNotifier = SubmitOrderChangeNotifier(orderService);

    WalletChangeNotifier walletChangeNotifier = WalletChangeNotifier(walletService);

    PositionChangeNotifier positionChangeNotifier = PositionChangeNotifier(positionService);

    TenTenOneConfigChangeNotifier configChangeNotifier =
        TenTenOneConfigChangeNotifier(channelConstraintsService);

    DlcChannelChangeNotifier dlcChannelChangeNotifier = DlcChannelChangeNotifier(dlcChannelService);
    dlcChannelChangeNotifier.initialize();

    TradeChangeNotifier tradeChangeNotifier = TradeChangeNotifier(tradeService);

    FundingRateChangeNotifier fundingRateChangeNotifier = FundingRateChangeNotifier();

    final tradeValuesChangeNotifier = TradeValuesChangeNotifier(tradeValueService);

    const askPrice = 30000.0;
    const bidPrice = 30000.0;

    // We have to have current price, otherwise we can't take order
    positionChangeNotifier.askPrice = askPrice;
    positionChangeNotifier.bidPrice = bidPrice;
    tradeValuesChangeNotifier.updatePrice(askPrice, Direction.short);
    tradeValuesChangeNotifier.updatePrice(bidPrice, Direction.long);

    await tester.pumpWidget(MultiProvider(
        providers: [
          ChangeNotifierProvider(create: (context) => tradeValuesChangeNotifier),
          ChangeNotifierProvider(create: (context) => submitOrderChangeNotifier),
          ChangeNotifierProvider(create: (context) => OrderChangeNotifier(orderService)),
          ChangeNotifierProvider(create: (context) => positionChangeNotifier),
          ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
          ChangeNotifierProvider(create: (context) => walletChangeNotifier),
          ChangeNotifierProvider(create: (context) => configChangeNotifier),
          ChangeNotifierProvider(create: (context) => dlcChannelChangeNotifier),
          ChangeNotifierProvider(create: (context) => tradeChangeNotifier),
          ChangeNotifierProvider(create: (context) => fundingRateChangeNotifier),
        ],
        child: TestWrapperWithTradeTheme(
          router: buildGoRouterMock(TradeScreen.route),
          child: const TradeScreen(),
        )));

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

void verifyMarginFieldValueSet(WidgetTester tester, Amount mockedDefaultMargin) {
  var marginFieldWidget = find.byKey(tradeButtonSheetMarginField);
  expect(marginFieldWidget, findsOneWidget);
  logger.i("Margin field found");
  final amountField = tester.widget<AmountTextField>(marginFieldWidget);
  expect(amountField.value, mockedDefaultMargin);
  logger.i("Margin field set correctly to $mockedDefaultMargin");
}

String assertPrice(WidgetTester tester, Finder byKey, String priceString) {
  final textWidget = tester.widget<RichText>(byKey);
  var text = textWidget.text as TextSpan;
  var children = text.children!.first;
  var plainText = children.toPlainText();
  expect(plainText, priceString);
  return plainText;
}
