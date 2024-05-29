import 'dart:math';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/tentenone_config_change_notifier.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/dlc_channel_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/channel_creation_flow/channel_funding_screen.dart';
import 'package:get_10101/features/trade/channel_creation_flow/custom_framed_container.dart';
import 'package:get_10101/features/trade/channel_creation_flow/fee_expansion_widget.dart';
import 'package:get_10101/features/trade/domain/channel_opening_params.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/constants.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';
import 'package:syncfusion_flutter_core/theme.dart' as slider_theme;
import 'package:syncfusion_flutter_sliders/sliders.dart';
import 'package:url_launcher/url_launcher.dart';

class ChannelConfigurationScreen extends StatelessWidget {
  static const route = "/channelconfiguration";
  static const label = "Channel Configuration";
  final Direction direction;

  const ChannelConfigurationScreen({super.key, required this.direction});

  @override
  Widget build(BuildContext context) {
    final tradeValues = context.read<TradeValuesChangeNotifier>().fromDirection(direction);
    return ChannelConfiguration(tradeValues: tradeValues);
  }
}

class ChannelConfiguration extends StatefulWidget {
  final TradeValues tradeValues;

  const ChannelConfiguration({super.key, required this.tradeValues});

  @override
  State<ChannelConfiguration> createState() => _ChannelConfiguration();
}

class _ChannelConfiguration extends State<ChannelConfiguration> {
  final TextEditingController _collateralController = TextEditingController();

  late final TenTenOneConfigChangeNotifier tentenoneConfigChangeNotifier;
  late final DlcChannelChangeNotifier dlcChannelChangeNotifier;

  bool useInnerWallet = false;

  Amount minMargin = Amount.zero();
  Amount counterpartyMargin = Amount.zero();
  Amount ownTotalCollateral = Amount.zero();
  Amount counterpartyCollateral = Amount.zero();

  double counterpartyLeverage = 1;

  Amount maxOnChainSpending = Amount.zero();
  Amount maxCounterpartyCollateral = Amount.zero();

  Amount orderMatchingFees = Amount.zero();

  Amount channelFeeReserve = Amount.zero();

  Amount fundingTxFee = Amount.zero();

  final _formKey = GlobalKey<FormState>();

  Usd quantity = Usd.zero();

  Leverage leverage = Leverage(0);

  double liquidationPrice = 0.0;

  bool fundWithWalletEnabled = true;

  Amount maxUsableOnChainBalance = Amount.zero();
  int maxCounterpartyCollateralSats = 0;

  Amount fundingTxFeeWithBuffer = Amount.zero();

  /// The minimum reserve the trader has to put into the channel.
  /// This is needed as the price might move before the external funding is found. To ensure
  /// that the order gets executed.
  ///
  /// TODO(holzeis): This won't be necessary anymore once we implement margin orders for externally
  /// funded positions.
  final minTraderReserveSats = 15000;

  bool externalFundingChannelButtonPressed = false;

  @override
  void initState() {
    super.initState();

    tentenoneConfigChangeNotifier = context.read<TenTenOneConfigChangeNotifier>();
    var tradeConstraints = tentenoneConfigChangeNotifier.channelInfoService.getTradeConstraints();

    DlcChannelService dlcChannelService =
        context.read<DlcChannelChangeNotifier>().dlcChannelService;

    quantity = widget.tradeValues.quantity;
    leverage = widget.tradeValues.leverage;
    liquidationPrice = widget.tradeValues.liquidationPrice ?? 0.0;

    maxCounterpartyCollateral = Amount(tradeConstraints.maxCounterpartyBalanceSats);

    maxOnChainSpending = Amount(tradeConstraints.maxLocalBalanceSats);
    counterpartyLeverage = tradeConstraints.coordinatorLeverage;

    counterpartyMargin = widget.tradeValues.calculateMargin(Leverage(counterpartyLeverage));

    minMargin = Amount(max(tradeConstraints.minMargin, widget.tradeValues.margin?.sats ?? 0));

    ownTotalCollateral = tradeConstraints.minMargin > widget.tradeValues.margin!.sats
        ? Amount(tradeConstraints.minMargin)
        : widget.tradeValues.margin!;

    _collateralController.text = ownTotalCollateral.formatted();

    orderMatchingFees = widget.tradeValues.fee ?? Amount.zero();

    updateCounterpartyCollateral();

    channelFeeReserve = dlcChannelService.getEstimatedChannelFeeReserve();

    fundingTxFee = dlcChannelService.getEstimatedFundingTxFee();

    // We add a buffer because the `fundingTxFee` is just an estimate. This
    // estimate will undershoot if we end up using more inputs or change
    // outputs.
    fundingTxFeeWithBuffer = Amount(fundingTxFee.sats * 2);

    maxUsableOnChainBalance =
        maxOnChainSpending - orderMatchingFees - fundingTxFeeWithBuffer - channelFeeReserve;

    maxCounterpartyCollateralSats = (maxCounterpartyCollateral.sats * counterpartyLeverage).toInt();

    fundWithWalletEnabled = maxUsableOnChainBalance.sats >= ownTotalCollateral.sats;

    // We add this so that the confirmation slider can be enabled immediately
    // _if_ the form is already valid. Otherwise we have to wait for the user to
    // interact with the form.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      setState(() {
        _formKey.currentState?.validate();
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    final tradeValueChangeNotifier = context.read<TradeValuesChangeNotifier>();
    final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();

    Color confirmationSliderColor =
        widget.tradeValues.direction == Direction.long ? tradeTheme.buy : tradeTheme.sell;

    Amount orderMatchingFee =
        tradeValueChangeNotifier.orderMatchingFee(widget.tradeValues.direction) ?? Amount.zero();

    Amount totalFee = orderMatchingFee.add(fundingTxFee).add(channelFeeReserve);
    Amount totalAmountToBeFunded = ownTotalCollateral.add(totalFee);

    Amount traderMargin = widget.tradeValues.margin ?? Amount.zero();

    Amount traderReserve = ownTotalCollateral - traderMargin;

    if (traderReserve.sats < minTraderReserveSats) {
      totalAmountToBeFunded =
          totalAmountToBeFunded.add(Amount(minTraderReserveSats) - traderReserve);
    }

    // The user needs to bring in a minimum reserve to cover for price movements.
    // TODO(holzeis): remove once we have margin orders.
    traderReserve = Amount(max(minTraderReserveSats, traderReserve.sats));

    return Scaffold(
      body: SafeArea(
        child: Form(
          key: _formKey,
          child: Container(
            padding: const EdgeInsets.only(top: 20, left: 15, right: 15),
            child: Column(
              children: [
                Column(
                  children: [
                    Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Expanded(
                          child: Stack(
                            children: [
                              GestureDetector(
                                child: Container(
                                    alignment: AlignmentDirectional.topStart,
                                    decoration: BoxDecoration(
                                        color: Colors.transparent,
                                        borderRadius: BorderRadius.circular(10)),
                                    width: 70,
                                    child: const Icon(
                                      Icons.arrow_back_ios_new_rounded,
                                      size: 22,
                                    )),
                                onTap: () {
                                  GoRouter.of(context).go(TradeScreen.route);
                                },
                              ),
                              const Row(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  Text(
                                    "Fund Channel",
                                    style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                    Center(
                      child: Text.rich(
                        textAlign: TextAlign.justify,
                        TextSpan(
                          children: [
                            TextSpan(
                              text:
                                  "🔔 This is your first trade and you do not have a channel yet. "
                                  "In 10101 we use DLC-Channels for fast and low-cost off-chain trading. "
                                  "You can read more about this technology ",
                              style: DefaultTextStyle.of(context).style,
                            ),
                            TextSpan(
                              text: 'here.',
                              style: const TextStyle(
                                color: tenTenOnePurple,
                                decoration: TextDecoration.underline,
                              ),
                              recognizer: TapGestureRecognizer()
                                ..onTap = () async {
                                  final httpsUri = Uri(
                                      scheme: 'https',
                                      host: '10101.finance',
                                      path: '/blog/dlc-channels-demystified');

                                  canLaunchUrl(httpsUri).then((canLaunch) async {
                                    if (canLaunch) {
                                      launchUrl(httpsUri, mode: LaunchMode.externalApplication);
                                    } else {
                                      showSnackBar(
                                          ScaffoldMessenger.of(context), "Failed to open link");
                                    }
                                  });
                                },
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
                Expanded(
                  child: Container(),
                ),
                Column(
                  children: [
                    CustomFramedContainer(
                        text: 'Channel size',
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Padding(
                              padding: const EdgeInsets.only(top: 5),
                              child: slider_theme.SfSliderTheme(
                                data: slider_theme.SfSliderThemeData(
                                  activeLabelStyle:
                                      const TextStyle(color: Colors.black, fontSize: 12),
                                  inactiveLabelStyle:
                                      const TextStyle(color: Colors.black, fontSize: 12),
                                  activeTrackColor: tenTenOnePurple.shade50,
                                  inactiveTrackColor: tenTenOnePurple.shade50,
                                  tickOffset: const Offset(0.0, 10.0),
                                ),
                                child: SfSlider(
                                  // TODO(bonomat): don't hard code this value
                                  min: 250000,
                                  max: maxCounterpartyCollateralSats,
                                  value: ownTotalCollateral.sats,
                                  stepSize: 100000,
                                  interval: 10000,
                                  showTicks: false,
                                  showLabels: true,
                                  enableTooltip: true,
                                  labelFormatterCallback:
                                      (dynamic actualValue, String formattedText) {
                                    if (actualValue == 250000) {
                                      return "Min";
                                    }

                                    if (actualValue == maxCounterpartyCollateralSats) {
                                      return "Max";
                                    }

                                    return "";
                                  },
                                  tooltipShape: const SfPaddleTooltipShape(),
                                  tooltipTextFormatterCallback:
                                      (dynamic actualValue, String formattedText) {
                                    return "${(actualValue as double).toInt()} sats";
                                  },
                                  onChanged: (dynamic value) {
                                    setState(() {
                                      if (value < minMargin.sats) {
                                        value = minMargin.sats.toDouble();
                                      }

                                      ownTotalCollateral = Amount((value as double).toInt());
                                      fundWithWalletEnabled =
                                          maxUsableOnChainBalance.sats >= ownTotalCollateral.sats;
                                      if (!fundWithWalletEnabled) {
                                        useInnerWallet = false;
                                      }

                                      updateCounterpartyCollateral();
                                    });
                                  },
                                ),
                              ),
                            ),
                            Padding(
                              padding:
                                  const EdgeInsets.only(top: 15, bottom: 5, left: 10, right: 10),
                              child: ValueDataRow(
                                  type: ValueType.amount,
                                  value: counterpartyCollateral,
                                  label: 'Win up to'),
                            )
                          ],
                        )),
                    CustomFramedContainer(
                        text: 'Order details',
                        child: Padding(
                          padding: const EdgeInsets.only(top: 15, bottom: 5, left: 10, right: 10),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              ValueDataRow(
                                  type: ValueType.fiat,
                                  value: quantity.asDouble(),
                                  label: 'Quantity'),
                              ValueDataRow(
                                  type: ValueType.text,
                                  value: leverage.formatted(),
                                  label: 'Leverage'),
                              ValueDataRow(
                                  type: ValueType.fiat,
                                  value: liquidationPrice,
                                  label: 'Liquidation price'),
                            ],
                          ),
                        )),
                    CustomFramedContainer(
                        text: 'Order cost',
                        child: Padding(
                          padding: const EdgeInsets.only(top: 15, bottom: 5, left: 10, right: 10),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              ValueDataRow(
                                  type: ValueType.amount, value: traderMargin, label: 'Margin'),
                              ValueDataRow(
                                  type: ValueType.amount, value: traderReserve, label: 'Reserve'),
                              FeeExpansionTile(
                                  value: totalFee,
                                  orderMatchingFee: orderMatchingFee,
                                  fundingTxFee: fundingTxFee,
                                  channelFeeReserve: channelFeeReserve),
                              const Divider(),
                              ValueDataRow(
                                  type: ValueType.amount,
                                  value: totalAmountToBeFunded,
                                  label: "Total"),
                            ],
                          ),
                        )),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.start,
                      children: [
                        Checkbox(
                          key: tradeScreenBottomSheetChannelConfigurationFundWithWalletCheckBox,
                          value: useInnerWallet,
                          onChanged: fundWithWalletEnabled
                              ? (bool? value) {
                                  setState(() {
                                    useInnerWallet = value ?? false;
                                  });
                                }
                              : null,
                        ),
                        Text(
                          "Fund with internal 10101 wallet",
                          style:
                              TextStyle(color: fundWithWalletEnabled ? Colors.black : Colors.grey),
                        )
                      ],
                    ),
                    SizedBox(
                      height: 50,
                      child: Visibility(
                        visible: useInnerWallet,
                        replacement: Padding(
                            padding: const EdgeInsets.only(top: 1, left: 8, right: 8, bottom: 8),
                            child: ElevatedButton.icon(
                                key: tradeScreenBottomSheetChannelConfigurationConfirmButton,
                                onPressed: _formKey.currentState != null &&
                                        _formKey.currentState!.validate() &&
                                        !externalFundingChannelButtonPressed
                                    ? () async {
                                        logger.d(
                                            "Submitting an order with ownTotalCollateral: $ownTotalCollateral orderMatchingFee: $orderMatchingFee, fundingTxFee: $fundingTxFee, channelFeeReserve: $channelFeeReserve, counterpartyCollateral: $counterpartyCollateral, ownMargin: ${widget.tradeValues.margin}");

                                        setState(() => externalFundingChannelButtonPressed = true);

                                        // TODO(holzeis): The coordinator leverage should not be hard coded here.
                                        final coordinatorCollateral =
                                            widget.tradeValues.calculateMargin(Leverage(2.0));

                                        final coordinatorReserve = max(0,
                                            counterpartyCollateral.sub(coordinatorCollateral).sats);
                                        final traderReserve = max(
                                            minTraderReserveSats,
                                            ownTotalCollateral
                                                .sub(widget.tradeValues.margin!)
                                                .sats);

                                        await submitOrderChangeNotifier
                                            .submitUnfundedOrder(
                                                widget.tradeValues,
                                                ChannelOpeningParams(
                                                    coordinatorReserve: Amount(coordinatorReserve),
                                                    traderReserve: Amount(traderReserve)))
                                            .then((ExternalFunding funding) {
                                          externalFundingChannelButtonPressed = false;
                                          GoRouter.of(context).push(ChannelFundingScreen.route,
                                              extra: {
                                                "funding": funding,
                                                "amount": totalAmountToBeFunded
                                              });
                                        }).onError((error, stackTrace) {
                                          setState(
                                              () => externalFundingChannelButtonPressed = false);
                                          logger.e("Failed at submitting unfunded order $error");
                                          final messenger = ScaffoldMessenger.of(context);
                                          showSnackBar(messenger,
                                              "Failed creating order ${error.toString()}");
                                        });
                                      }
                                    : null,
                                icon: externalFundingChannelButtonPressed
                                    ? const SizedBox(
                                        width: 15,
                                        height: 15,
                                        child: CircularProgressIndicator(
                                            color: Colors.white, strokeWidth: 2))
                                    : Container(),
                                label: const Text(
                                  "Next",
                                  style: TextStyle(color: Colors.white),
                                ),
                                style: ElevatedButton.styleFrom(
                                    minimumSize: const Size.fromHeight(50),
                                    disabledBackgroundColor: tenTenOnePurple.shade200))),
                        child: Padding(
                          padding: const EdgeInsets.only(top: 1, left: 8, right: 8, bottom: 8),
                          child: ConfirmationSlider(
                            key: tradeScreenBottomSheetChannelConfigurationConfirmSlider,
                            text: "Swipe to confirm ${widget.tradeValues.direction.nameU}",
                            textStyle: TextStyle(color: confirmationSliderColor),
                            height: 40,
                            foregroundColor: confirmationSliderColor,
                            sliderButtonContent: const Icon(
                              Icons.chevron_right,
                              color: Colors.white,
                              size: 20,
                            ),
                            onConfirmation: () async {
                              logger.d("Submitting new order with "
                                  "quantity: ${widget.tradeValues.quantity}, "
                                  "leverage: ${widget.tradeValues.leverage.formatted()}, "
                                  "direction: ${widget.tradeValues.direction}, "
                                  "liquidationPrice: ${widget.tradeValues.liquidationPrice}, "
                                  "margin: ${widget.tradeValues.margin}, "
                                  "ownTotalCollateral: $ownTotalCollateral, "
                                  "counterpartyCollateral: $counterpartyCollateral, "
                                  "");
                              submitOrderChangeNotifier
                                  .submitOrder(widget.tradeValues,
                                      channelOpeningParams: ChannelOpeningParams(
                                          coordinatorReserve: counterpartyCollateral,
                                          traderReserve: ownTotalCollateral))
                                  .onError((error, stackTrace) {
                                logger.e("Failed creating new channel due to $error");
                                final messenger = ScaffoldMessenger.of(context);
                                showSnackBar(messenger, "Failed creating order ${e.toString()}");
                              }).then((ignored) => GoRouter.of(context).go(TradeScreen.route));
                            },
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  void updateCounterpartyCollateral() {
    final collateral = (ownTotalCollateral.sats / counterpartyLeverage).floor();
    counterpartyCollateral =
        counterpartyMargin.sats > collateral ? counterpartyMargin : Amount(collateral);
  }
}

formatNumber(dynamic myNumber) {
  // Convert number into a string if it was not a string previously
  String stringNumber = myNumber.toString();

  // Convert number into double to be formatted.
  // Default to zero if unable to do so
  double doubleNumber = double.tryParse(stringNumber) ?? 0;

  // Set number format to use
  NumberFormat numberFormat = NumberFormat.compact();

  return numberFormat.format(doubleNumber);
}

int roundToNearestThousand(int value) {
  return ((value + 500) ~/ 1000) * 1000;
}
