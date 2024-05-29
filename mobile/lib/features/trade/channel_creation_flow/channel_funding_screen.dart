import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/bitcoin_balance_field.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/countdown.dart';
import 'package:get_10101/common/custom_qr_code.dart';
import 'package:get_10101/common/domain/funding_channel_task.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/funding_channel_task_change_notifier.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/channel_creation_flow/channel_configuration_screen.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

// TODO: Fetch from backend.
Amount openingFee = Amount(0);

class ChannelFundingScreen extends StatelessWidget {
  static const route = "${ChannelConfigurationScreen.route}/$subRouteName";
  static const subRouteName = "fund_tx";
  final Amount amount;
  final ExternalFunding funding;

  const ChannelFundingScreen({
    super.key,
    required this.amount,
    required this.funding,
  });

  @override
  Widget build(BuildContext context) {
    return ChannelFunding(
      amount: amount,
      funding: funding,
    );
  }
}

enum FundingType {
  lightning,
  onchain,
  unified,
  external,
}

class ChannelFunding extends StatefulWidget {
  final Amount amount;
  final ExternalFunding funding;

  const ChannelFunding({super.key, required this.amount, required this.funding});

  @override
  State<ChannelFunding> createState() => _ChannelFunding();
}

class _ChannelFunding extends State<ChannelFunding> {
  FundingType selectedBox = FundingType.onchain;

  // TODO(holzeis): It would be nicer if this would come directly from the invoice.
  final expiry = DateTime.timestamp().second + 300;

  @override
  Widget build(BuildContext context) {
    final status = context.watch<FundingChannelChangeNotifier>().status;
    final orderCreated = status == FundingChannelTaskStatus.orderCreated;

    final qrCode = switch (selectedBox) {
      FundingType.lightning => CustomQrCode(
          data: widget.funding.paymentRequest ?? "https://x.com/get10101",
          embeddedImage: widget.funding.paymentRequest != null
              ? const AssetImage("assets/10101_logo_icon_white_background.png")
              : const AssetImage("assets/coming_soon.png"),
          embeddedImageSizeHeight: widget.funding.paymentRequest != null ? 50 : 350,
          embeddedImageSizeWidth: widget.funding.paymentRequest != null ? 50 : 350,
          dimension: 300,
        ),
      FundingType.onchain => CustomQrCode(
          // TODO: creating a bip21 qr code should be generic once we support other desposit methods
          data: "bitcoin:${widget.funding.bitcoinAddress}?amount=${widget.amount.btc.toString()}",
          embeddedImage: const AssetImage("assets/10101_logo_icon_white_background.png"),
          dimension: 300,
        ),
      FundingType.unified || FundingType.external => const CustomQrCode(
          data: "https://x.com/get10101",
          embeddedImage: AssetImage("assets/coming_soon.png"),
          embeddedImageSizeHeight: 350,
          embeddedImageSizeWidth: 350,
          dimension: 300,
        )
    };

    final qrCodeSubTitle = switch (selectedBox) {
      FundingType.lightning =>
        widget.funding.paymentRequest ?? "Currently not available. Try again later.",
      FundingType.onchain => widget.funding.bitcoinAddress,
      FundingType.unified || FundingType.external => "Follow us on social media for updates.",
    };

    return Scaffold(
      body: SafeArea(
        child: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
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
                                context
                                    .read<SubmitOrderChangeNotifier>()
                                    .orderService
                                    .abortUnfundedChannelOpeningMarketOrder()
                                    .then((value) {
                                  if (orderCreated) {
                                    GoRouter.of(context).go(TradeScreen.route);
                                  } else {
                                    GoRouter.of(context).pop();
                                  }
                                });
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
                ],
              ),
              // QR code and content field
              Column(
                children: [
                  Container(
                    width: double.infinity,
                    margin: const EdgeInsets.fromLTRB(0, 20, 0, 20),
                    padding: const EdgeInsets.only(top: 10, left: 0, right: 0),
                    decoration: BoxDecoration(
                      color: Colors.grey.shade100,
                      border: Border.all(color: Colors.grey, width: 1),
                      borderRadius: BorderRadius.circular(10),
                      shape: BoxShape.rectangle,
                    ),
                    child: Center(
                      child: Column(
                        children: [
                          GestureDetector(
                            onTap: () {
                              Clipboard.setData(ClipboardData(text: widget.amount.btc.toString()))
                                  .then((_) {
                                showSnackBar(ScaffoldMessenger.of(context),
                                    "Copied amount: ${widget.amount}");
                              });
                            },
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              crossAxisAlignment: CrossAxisAlignment.end,
                              children: [
                                BitcoinBalanceField(bitcoinBalance: widget.amount),
                              ],
                            ),
                          ),
                          GestureDetector(
                            onTap: () {
                              Clipboard.setData(ClipboardData(text: qrCode.data)).then((_) {
                                showSnackBar(
                                    ScaffoldMessenger.of(context), "Copied: ${qrCode.data}");
                              });
                            },
                            child: Padding(
                              padding: const EdgeInsets.all(8.0),
                              child: qrCode,
                            ),
                          ),
                          LayoutBuilder(
                            builder: (BuildContext context, BoxConstraints constraints) {
                              return FittedBox(
                                fit: BoxFit.scaleDown,
                                child: Padding(
                                  padding: const EdgeInsets.only(left: 10.0, right: 10.0),
                                  child: GestureDetector(
                                    onTap: () {
                                      Clipboard.setData(ClipboardData(text: qrCodeSubTitle))
                                          .then((_) {
                                        showSnackBar(ScaffoldMessenger.of(context),
                                            "Copied: $qrCodeSubTitle");
                                      });
                                    },
                                    child: Column(
                                      children: [
                                        Text(
                                          truncateWithEllipsis(44, qrCodeSubTitle),
                                          style: const TextStyle(fontSize: 14),
                                          textAlign: TextAlign.center,
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                        if (selectedBox == FundingType.lightning)
                                          Padding(
                                            padding: const EdgeInsets.only(top: 2),
                                            child: Countdown(
                                                start: expiry > DateTime.timestamp().second
                                                    ? expiry - DateTime.timestamp().second
                                                    : 0),
                                          ),
                                      ],
                                    ),
                                  ),
                                ),
                              );
                            },
                          ),
                          const SizedBox(
                            height: 10,
                          ),
                        ],
                      ),
                    ),
                  )
                ],
              ),
              // information text about the tx status
              Expanded(child: buildInfoBox(status, selectedBox)),
              Padding(
                  padding: const EdgeInsets.only(top: 1, left: 8, right: 8, bottom: 8),
                  child: orderCreated
                      ? ElevatedButton(
                          onPressed: () {
                            GoRouter.of(context).go(TradeScreen.route);
                          },
                          style: ElevatedButton.styleFrom(
                              minimumSize: const Size.fromHeight(50),
                              backgroundColor: tenTenOnePurple),
                          child: const Text(
                            "Home",
                            style: TextStyle(color: Colors.white),
                          ),
                        )
                      : buildButtonRow()),
            ],
          ),
        ),
      ),
    );
  }

  Row buildButtonRow() {
    return Row(
      children: [
        Expanded(
          child: ClickableBox(
            text: "Unified",
            image: const Icon(BitcoinIcons.bitcoin_circle_outline),
            isSelected: selectedBox == FundingType.unified,
            onTap: () {
              setState(() {
                selectedBox = FundingType.unified;
              });
            },
          ),
        ),
        Expanded(
          child: ClickableBox(
            text: "Lightning",
            image: const Icon(BitcoinIcons.lightning_outline),
            isSelected: selectedBox == FundingType.lightning,
            onTap: () {
              setState(() {
                selectedBox = FundingType.lightning;
              });
            },
          ),
        ),
        Expanded(
          child: ClickableBox(
            text: "On-chain",
            image: const Icon(BitcoinIcons.link_outline),
            isSelected: selectedBox == FundingType.onchain,
            onTap: () {
              setState(() {
                selectedBox = FundingType.onchain;
              });
            },
          ),
        ),
        Expanded(
          child: ClickableBox(
            text: "External",
            image: const Icon(BitcoinIcons.wallet),
            isSelected: selectedBox == FundingType.external,
            onTap: () {
              setState(() {
                selectedBox = FundingType.external;
              });
            },
          ),
        )
      ],
    );
  }

  Column buildInfoBox(FundingChannelTaskStatus? value, FundingType selectedBox) {
    String transactionStatusText = "Waiting for payment...";
    String transactionStatusInformationText =
        "Please wait. If you leave now, your position won’t be opened when the funds arrive.";

    Widget loadingWidget = Container();

    switch (selectedBox) {
      case FundingType.onchain:
        switch (value) {
          case null:
          case FundingChannelTaskStatus.pending:
            loadingWidget = const RotatingIcon(icon: Icons.sync);
            break;
          case FundingChannelTaskStatus.funded:
            transactionStatusText = "Address funded";
            loadingWidget = const RotatingIcon(icon: BitcoinIcons.bitcoin);
            break;
          case FundingChannelTaskStatus.orderCreated:
            transactionStatusText = "Order successfully created";
            transactionStatusInformationText = "";
            loadingWidget = const Icon(
              Icons.check,
              size: 20.0,
              color: tenTenOnePurple,
            );
            break;
          case FundingChannelTaskStatus.failed:
            loadingWidget = const Icon(
              Icons.error,
              size: 20.0,
              color: tenTenOnePurple,
            );
            break;
        }
      case FundingType.lightning:
        switch (value) {
          case null:
          case FundingChannelTaskStatus.pending:
            loadingWidget = const RotatingIcon(icon: Icons.sync);
            break;
          case FundingChannelTaskStatus.funded:
            transactionStatusText = "Lightning payment received";
            loadingWidget = const RotatingIcon(icon: BitcoinIcons.bitcoin);
            break;
          case FundingChannelTaskStatus.orderCreated:
            transactionStatusText = "Order successfully created";
            transactionStatusInformationText = "";
            loadingWidget = const Icon(
              Icons.check,
              size: 20.0,
              color: tenTenOnePurple,
            );
            break;
          case FundingChannelTaskStatus.failed:
            loadingWidget = const Icon(
              Icons.error,
              size: 20.0,
              color: tenTenOnePurple,
            );
            break;
        }
      default:
        break;
    }

    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(transactionStatusText),
            loadingWidget,
          ],
        ),
        const SizedBox(
          height: 5,
        ),
        Text(
          transactionStatusInformationText,
          textAlign: TextAlign.center,
        )
      ],
    );
  }
}

class ClickableBox extends StatelessWidget {
  final String text;
  final Widget image;
  final bool isSelected;
  final VoidCallback onTap;

  const ClickableBox({
    Key? key,
    required this.text,
    required this.image,
    required this.isSelected,
    required this.onTap,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        decoration: BoxDecoration(
          color: isSelected ? tenTenOnePurple.shade100 : Colors.transparent,
          borderRadius: BorderRadius.circular(10),
        ),
        padding: const EdgeInsets.only(left: 10, right: 10, top: 2, bottom: 2),
        child: Column(
          children: [
            image,
            const SizedBox(height: 1),
            LayoutBuilder(
              builder: (BuildContext context, BoxConstraints constraints) {
                return FittedBox(
                  fit: BoxFit.scaleDown,
                  child: Text(
                    text,
                    style: const TextStyle(
                      color: Colors.black,
                      fontSize: 16,
                    ),
                    textAlign: TextAlign.center,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                );
              },
            )
          ],
        ),
      ),
    );
  }
}

class RotatingIcon extends StatefulWidget {
  final IconData icon;

  const RotatingIcon({super.key, required this.icon});

  @override
  State<StatefulWidget> createState() => _RotatingIconState();
}

class _RotatingIconState extends State<RotatingIcon> with SingleTickerProviderStateMixin {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: const Duration(seconds: 2),
      vsync: this,
    )..repeat(); // Repeats the animation indefinitely
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return RotationTransition(
      turns: _controller,
      child: Icon(
        widget.icon,
        size: 20.0,
        color: tenTenOnePurple,
      ),
    );
  }
}
