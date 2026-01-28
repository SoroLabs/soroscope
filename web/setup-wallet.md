# Stellar Wallet Integration Setup

## Installation

Run the following command to install the required dependencies:

```bash
cd web
npm install
# or
pnpm install
```

## Dependencies

- `@creit.tech/stellar-wallets-kit@^1.9.5` - Stellar wallet integration
- `framer-motion@^11.0.0` - Smooth animations
- `lucide-react@^0.562.0` - Icons

## Features Implemented

✅ **Wallet Connection Logic**

- Stellar Wallets SDK integration (v1.9.5)
- Support for multiple wallet providers (Freighter, Albedo, xBull, Rabet, Lobstr)
- Connection state persistence across page refreshes
- Proper error handling and user feedback

✅ **UI Components**

- Connect Wallet button in top-right header
- Wallet selection modal with provider options
- Connected state showing truncated public key (GA...4R3)
- Disconnect functionality with dropdown
- Error messages for failed connections

✅ **State Management**

- React Context for global wallet state
- Local storage persistence
- Connection status tracking
- Error state management

## Usage

1. The wallet connect button appears in the top-right of the dashboard
2. Click to open the wallet selection modal
3. Choose your preferred wallet provider (Freighter recommended)
4. Approve the connection in your wallet extension
5. Your truncated address will be displayed
6. Click the address to access disconnect option

## Supported Wallets

- **Freighter** (Primary) - Browser extension wallet
- **Albedo** - Web-based wallet
- **xBull** - Mobile and browser wallet
- **Rabet** - Browser extension wallet
- **Lobstr** - Mobile wallet

## Network Configuration

Currently configured for **Stellar Testnet**. To switch to mainnet, update the `WalletNetwork` in `context/WalletContext.tsx`:

```typescript
network: WalletNetwork.PUBLIC, // for mainnet
```

## Error Handling

The integration includes comprehensive error handling:

- Wallet not installed detection
- Connection rejection handling
- Network errors
- User-friendly error messages in the modal

## Next Steps

The wallet integration is ready for:

- Smart contract interactions
- Transaction signing
- Asset management
- Inheritance contract deployment

All wallet functionality is accessible through the `useWallet()` hook in any component.

## Testing

To test the integration:

1. Install Freighter browser extension
2. Create a testnet account
3. Run the development server: `npm run dev`
4. Click "Connect Wallet" and test the connection flow
